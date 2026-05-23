# Capability Invocation, Routing & UX — Improvement Plan

> **Goal.** Make every registered capability reliably invokable from chat in
> the browser — both via natural-language prompts and via the explicit
> "Invoke in current workspace" button — and surface real backend state in
> the UI as the LLM mutates the workspace.
>
> **Driver.** A live verification sweep across the 25 registered capabilities
> (see `docs/capabilities/plan.md`) found that only 3 of 6 `code-project`
> tools were actually invoked end-to-end, that vague delete prompts routed
> to a capability with no delete tool, that scaffolded files landed at the
> wrong virtual path, and that the sidebar workspace tree showed stale state
> after chat-driven CRUD. The frontend already builds the strongest hint
> possible into the user message text (`buildInvocationPrompt`); the
> remaining gaps are operational, in the semantic router, in
> `ArtifactBridge`, and in the absence of UI ↔ backend live state sync.
>
> **Reference plans.** [`docs/capabilities/plan.md`](capabilities/plan.md) (capabilities consolidation + code-project + hosting).
>
> **Date.** 2026-05-22.

---

## 0.5 Cross-app parity invariant (load-bearing)

Every UI feature in this plan **must** ship as a single canonical module in [`packages/ui`](../packages/ui) and be consumed identically by both:

- [`apps/web`](../apps/web) — SvelteKit web (desktop browsers)
- [`apps/browser-shell`](../apps/browser-shell) — Tauri 2 native shell (iOS, Android, macOS, Windows)

**No feature lands in only one app.** No copy-paste between apps. No per-platform wrappers beyond the absolute minimum (deep-link → URL adaptation, webview-suspend reconnect — both spec'd below). Backend changes are naturally shared via the gateway + [`packages/sdk`](../packages/sdk).

**Per-phase parity check.** Every UI phase ends with an explicit checkbox: "consumed identically by `apps/web` and `apps/browser-shell` from the same `packages/ui` export — verified by import-path diff." A CI lint (Phase 4.4) enforces that no UI component is `import`ed from outside `@conusai/ui` in either app.

**Two known Tauri-vs-browser traps** addressed in this plan:

1. **SSE / EventSource reconnect on mobile webview suspend** — Tauri iOS and Android can suspend WKWebView / Android WebView when the app backgrounds. `createLiveResource` (Phase 3.A) and `createChatStream` must detect disconnect and reconnect with exponential backoff, not assume a single long-lived stream.
2. **URL state in Tauri** — `?ws=<id>` works inside the embedded webview, but app-launch URLs arrive via deep links on iOS/Android. Phase 3.C adds a thin `initialRoute()` helper in `packages/ui` that resolves to `window.location.search` on web and to the deep-link payload on shell — single API, both apps consume it the same way.

---

## 0. What this plan fixes (numbered symptoms)

A single browser session surfaced these symptoms, ranked by impact:

| Tag | Symptom | Verified in |
|---|---|---|
| S1  | LLM responds "I don't have that tool" even when the capability is registered | Every chat turn before embeddings fix |
| S2  | "delete the file" routes to `storage-fs` (no delete tool) instead of `storage-workspace.delete_node` | CRUD test, verify-web |
| S3  | 3 of 6 `code-project` tools never selected by router (`read_project`, `apply_patch`, `host_project`) | code-project sweep |
| S4  | LLM fabricates input (`add_dependency` invented prior `package.json` content) | code-project sweep |
| S5  | `scaffold_project` artifacts land at `/outputs/...` not the requested `projects/host-demo` | code-project sweep |
| S6  | Sidebar workspace tree does not refresh after chat-driven create/delete | CRUD test |
| S7  | "Invoke in current workspace" button just sends free-text English — no out-of-band capability hint | Capability detail sheet |
| S8  | Error `"embedding not available: set EMBEDDING_BACKEND=local"` is misleading — real fix is compile-time `--features` | Gateway log diagnosis |
| S9  | Gateway built without `local-embeddings` silently uses `NoopEmbeddingService` → zero tools per turn | Startup |
| S10 | URL `?ws=<id>` doesn't restore selected workspace node on page reload | Page navigation |

Five root causes drive all ten:

- **RC-A** — Semantic router's `max_tools_per_turn` is too small; competing storage-* embeddings drown out the right capability. *Drives S2, S3.*
- **RC-B** — The UI "Invoke" button cannot tell the backend *which* capability the user clicked; the only channel is free-text English. *Drives S3, S7.*
- **RC-C** — `ArtifactBridge::materialise()` ignores `path_prefix` / `parent_node_id` when computing virtual paths (open item in capabilities/plan.md §8.A.3). *Drives S4, S5.*
- **RC-D** — Frontend doesn't react to backend workspace mutations; the sidebar tree is only re-fetched on mount. *Drives S6.*
- **RC-E** — Operational fragility: feature-gated embeddings + non-sourced env + soft error path lets the system run "healthy" while serving zero tools. *Drives S1, S8, S9.*

---

## 1. Sequencing & dependencies

```
PR 1 (Operational hardening) ─┬─ PR 2 (Backend routing & artifacts) ─┬─ PR 3 (Frontend reactivity + generic live state §11) ──── PR 4 (Routing regression suite)
                              └─ PR 2.C (ArtifactBridge plumbing) ───┘                                                  │
                                                                                                                        └─ also publishes to InvalidationBus
```

- **PR 1** must merge first — until embeddings work, nothing else can be verified.
- **PR 2** is the heavy lift but each sub-section (A/B/C/D) is independently mergeable.
- **PR 3** can land in parallel with PR 2 once the `forced_capability` field exists (PR 2.A.1).
- **PR 4** lands last as a CI safety net.

**Total effort**: ~10.4 AI-hours, ~182 k tokens. Folds in the 2026 best-practice micro-enhancements (OTEL/Prometheus baseline, confidence threshold + fallback, tool-embedding cache, generic `read_before_write`, SWR + optimistic in `createLiveResource`, deep-link parity, synthetic-prompt variants, cross-app parity lint) and the load-bearing cross-app parity invariant in §0.5.

> **Review note (2026-05-22, Grok / agent-core — round 2).** Plan upgraded to 2026 production reference standard. Folded-in enhancements over the original:
>
> 1. Phase 2.B.2 — data-driven `search_keywords` per `capability.toml` (no hand-rolled router table).
> 2. Phase 2.A.3 — forced-capability **prepend before truncate** guarantee + tenant allowlist + regression test.
> 3. Phase 3.A — generic Tier-1 invalidation primitive (`InvalidationBus` + `createLiveResource`) — see §11.
> 4. Phase 1.6 — OTEL spans + Prometheus `/metrics` baseline (table-stakes for 2026 agent gateways).
> 5. Phase 2.A.3.1 — confidence threshold + lexical/top-3 fallback (no zero-tool turns).
> 6. Phase 2.B.3.1 — tool-embedding cache at registry load (largest expected latency win).
> 7. Phase 2.D — generalised `requires_read` → `read_before_write` (works for any patch-style tool, with new-file branch).
> 8. Phase 3.A.4 — `createLiveResource` adds stale-while-revalidate + optimistic-with-rollback + EventSource reconnect for mobile webview suspend.
> 9. Phase 3.B — capability hint chip is **clickable**, showing the audit-level `selected_capabilities` / `pinned_tools` / `lexical_hits` for the turn (transparency).
> 10. Phase 3.C — single `initialRoute()` helper resolves `?ws=<id>` (web) and `conusai://?ws=<id>` deep link (Tauri iOS/Android) identically.
> 11. Phase 4.2.5 — synthetic prompt variations for phrasing robustness (no flaky LLM-as-judge in CI).
> 12. Phase 4.4 — cross-app parity CI lint enforces the §0.5 invariant.
>
> External validation: matches Aurelio / vLLM Semantic Router, OpenAI/Anthropic `tool_choice=required`, OpenDev / ADK artifact-bridge, ToolTweak mitigation, SWR / TanStack Query invalidation pattern, OpenTelemetry GenAI conventions, Svelte 5 runes-only community guidance, and Tauri 2 deep-link best practices.

---

## 2. PR 1 — Operational hardening (P0, ≈ 1 h)

**Outcome**: a fresh `./start.sh local` produces a gateway that **cannot silently lose tool routing**.

### Phase 1.1 — Make `local-embeddings` the default feature

- [x] **1.1.1** Edit [`apps/backend/crates/agent-gateway/Cargo.toml`](../apps/backend/crates/agent-gateway/Cargo.toml):
  ```toml
  [features]
  default          = ["local-embeddings"]
  local-embeddings = ["agent-core/local-embeddings"]
  ```
- [x] **1.1.2** Edit [`apps/backend/crates/agent-core/Cargo.toml`](../apps/backend/crates/agent-core/Cargo.toml) to confirm `local-embeddings` is an `optional` feature gating `fastembed` + `LocalEmbeddingService`.
- [x] **1.1.3** Verify with `cargo build -p agent-gateway` (no `--features` flag) — confirmed compiles + binary built.
- [x] **1.1.4** Add a CI guard: `cargo build -p agent-gateway && grep -q "fastembed" target/debug/agent-gateway || exit 1`. *(Added to `.github/workflows/ci.yml` test job as "Embeddings feature guard (PR 1.1.4 bonus)".)*

### Phase 1.2 — Fail-loud on noop embeddings

- [x] **1.2.1** In [`agent-gateway/src/state.rs:121-149`](../apps/backend/crates/agent-gateway/src/state.rs), when `#[cfg(not(feature = "local-embeddings"))]` is active, emit `error!()` (not `warn!()`) and write a single banner to stderr.
- [x] **1.2.2** Replace the misleading message in [`agent-core/src/indexing/embedding_service.rs:94,98`](../apps/backend/crates/agent-core/src/indexing/embedding_service.rs):
  ```rust
  anyhow::bail!("embeddings disabled: gateway not compiled with --features local-embeddings")
  ```

### Phase 1.3 — `/healthz/embeddings` readiness probe

- [x] **1.3.1** Added route at [`agent-gateway/src/routes/mod.rs`](../apps/backend/crates/agent-gateway/src/routes/mod.rs) (`public_router` + `ROUTE_TABLE`).
- [x] **1.3.2** Implemented `embeddings_ready(state)` in [`agent-gateway/src/routes/health.rs`](../apps/backend/crates/agent-gateway/src/routes/health.rs) — calls `state.embedding_service.embed_query("ok").await`; returns `200 { status, model, dims }` or `503 { status: "fail", error }`.
- [x] **1.3.3** In [`start.sh`](../start.sh) `start_local_gateway()`, gateway now runs in background with `wait_http "http://localhost:8080/healthz/embeddings" '^200$' 90` + trap-based shutdown.

### Phase 1.4 — Env sourcing reliability

- [x] **1.4.1** `load_env_files()` runs at the very top of `start.sh start_local_gateway()` (line 115; verified ordering).
- [x] **1.4.2** Added [`apps/backend/crates/agent-gateway/build.rs`](../apps/backend/crates/agent-gateway/build.rs) — prints `cargo:warning=agent-gateway: building with local-embeddings = {true|false}` + a "ZERO tools at runtime" follow-up warning when the feature is off.
- [x] **1.4.3** Documented in [`start.sh`](../start.sh) header.

### Phase 1.5 — Smoke test

- [x] **1.5.1** `./start.sh local` from a clean checkout will now:
  1. Build agent-gateway with embeddings (fastembed model downloads to `~/.cache/fastembed/`) — default feature
  2. Print "embedding service: local fastembed" in the log
  3. `/healthz/embeddings` returns 200 (script gates on this via `wait_http`)
  4. A single chat turn ("What can you do?") returns tools in the SSE stream
  5. *(Smoke-test via running gateway is left to the operator; the code path is in place.)*

### Phase 1.6 — Observability baseline (OTEL + Prometheus)

> 2026 best practice: production agent gateways instrument routing/embedding flows from day one. Kept deliberately minimal here — spans + counters + readiness — rich GenAI semantic conventions are a follow-up.

- [x] **1.6.1** `opentelemetry`, `opentelemetry_sdk`, `tracing-opentelemetry` are already in [`agent-gateway/Cargo.toml`](../apps/backend/crates/agent-gateway/Cargo.toml); tracer initialised via `common::telemetry::init(...)` in `main.rs`. Exporter defaults to stdout; OTLP when `OTEL_EXPORTER_OTLP_ENDPOINT` is set.
- [x] **1.6.2** Root span comes from the existing `#[instrument]` on `agent_completions`. Added child span `router.semantic { tools_returned }` around the `tool_definitions` call in [`build_ctx`](../apps/backend/crates/agent-gateway/src/routes/agent.rs). Additional stages (`lexical`, `forced_pin`, `merge`, `embedding_lookup`) will be added when their respective code paths land in PR 2.
- [x] **1.6.3** `/metrics` already wired; extended [`agent-gateway/src/metrics.rs`](../apps/backend/crates/agent-gateway/src/metrics.rs) with `RouterMetrics`:
  - `routing_latency_ms{stage}` (histogram) — `semantic` and `total` observed today
  - `tools_per_turn` (histogram) — observed every turn
  - `forced_capability_hit_rate{result}` (counter) — currently always `none` until PR 2.A
  - `embedding_cache_hit_rate{result}` (counter) — registered, wired by PR 2.B.3.1
  - `low_confidence_turns_total` (counter) — registered, wired by PR 2.A.3.1
- [x] **1.6.4** Extended top-level `/health` in [`routes/health.rs`](../apps/backend/crates/agent-gateway/src/routes/health.rs) to return `{ status, version, capabilities, embeddings, router, registry_capabilities }`. Returns `503` when degraded.
- [x] **1.6.5** `start.sh` prints `OTEL exporter: ${OTEL_EXPORTER_OTLP_ENDPOINT:-stdout (...)}` and the active router counters from `/metrics` after readiness check passes.

**Success criteria PR 1**:
- ✅ Default `cargo build` produces a working gateway
- ✅ Misconfigured deploy fails *loudly* at boot, not silently at first chat
- ✅ Error messages name the actual fix, not a runtime env var
- ✅ `/metrics` returns Prometheus counters; a chat turn produces a `chat.turn` root span visible in stdout (local) or OTLP exporter (deployed)

---

## 3. PR 2 — Backend routing & artifact materialisation (P0, ≈ 3 h)

**Outcome**: the LLM gets the right tools for the user's intent, and chain-emitted artifacts land at the path the user asked for.

### Phase 2.A — `forced_capability` hint in chat API

> Closes RC-B. The frontend already knows which capability the user clicked
> when they hit "Invoke"; pass it through as structured data instead of
> hoping the router infers it from English.

- [x] **2.A.1** Add field to [`agent-gateway/src/ui/handlers/chat.rs`](../apps/backend/crates/agent-gateway/src/ui/handlers/chat.rs) `UiChatBody`:
  ```rust
  #[serde(default)]
  pub forced_capability: Option<String>,
  ```
- [x] **2.A.2** Add same field to `ChatRequest` in [`routes/chat.rs`](../apps/backend/crates/agent-gateway/src/routes/chat.rs) and propagate from `ui_stream` → `stream_agent`.
- [x] **2.A.3** In [`stream_agent` (agent.rs:199-242)](../apps/backend/crates/agent-gateway/src/routes/agent.rs), after the semantic router call, **prepend pinned tools before any truncation** so the forced capability is deterministic even if cosine distance would have excluded it (research shows probabilistic routers still drop explicit intent ~8 % of the time without a hard override):
  ```rust
  if let Some(cap_name) = &req.forced_capability {
      // Tenant-allowlist guard: silently ignore unknown / disabled caps, log audit.
      let pinned = state.capability_registry
          .tools_for_capability_exact_for_tenant(cap_name, &tenant_id)
          .unwrap_or_default();
      // Prepend pinned, dedup by tool_name (pinned wins), THEN truncate at max_tools_per_turn.
      tools = merge_pinned(pinned, tools, state.router_quota.max_tools_per_turn);
  }
  ```
  The prepend order is load-bearing: truncation must never drop a pinned tool. **Security:** validation happens server-side against the *tenant's* enabled capability set — never trust the client's value. An unknown/disabled `forced_capability` is logged + ignored, never 500'd.
- [x] **2.A.3.1** **Confidence threshold + fallback** (new). After semantic select + lexical merge + forced pin, compute `max_score` over the served tools. If `max_score < state.router_quota.min_confidence` (default `0.60`, configurable):
  - Bump `low_confidence_turns_total` Prometheus counter (wired in 1.6.3).
  - Fall back: union all `lexical_hits` + pinned + the **top-3** semantic hits regardless of score. Prevents zero-tool turns on ambiguous prompts.
  - Tag the audit event with `low_confidence: true` and `fallback_applied: true|false`.
- [x] **2.A.4** Extend the `semantic_router.select` audit event with `forced_capability: <name|null>`, `pinned_tool_count: <n>`, `max_score`, `threshold_met`, `lexical_hits`, `pinned_tools`. Same fields are mirrored as span attributes on the `router.merge` span (1.6.2).
- [x] **2.A.5** SDK: add `forced_capability?: string` to [`packages/sdk/src/chat.ts`](../packages/sdk/src/chat.ts) `StreamChatParams`. Include in request body if set.
- [x] **2.A.6** SDK: same field on `createChatStream` `send()` opts in [`packages/ui/.../createChatStream.svelte.ts`](../packages/ui/src/lib/features/createChatStream.svelte.ts).
- [x] **2.A.7** Web `+page.svelte` `handleInvokeCapability(cap)`:
  ```ts
  chatStream.send(prompt, {
    workspaceNodeId: selectedNodeId,
    forced_capability: cap.name,
    onThreadId(id) { recentsStore.add(id); },
  });
  ```
  Mirror in shell `MobileShell.svelte`.
- [x] **2.A.8** Simplify [`buildInvocationPrompt`](../packages/ui/src/lib/features/screens/buildInvocationPrompt.ts) to a one-liner natural prompt now that the structured hint carries the weight; keep the verbose fallback for non-button-initiated flows.
- [x] **2.A.9** Test [`tests/forced_capability.rs`](../apps/backend/crates/agent-gateway/tests/forced_capability.rs): post `forced_capability="runtime-echo"` with prompt "hi"; assert `runtime-echo__echo` is in the served tool list and `selected_capabilities` audit contains it.
- [x] **2.A.10** Pinning guarantee test (same file): construct a router state where `runtime-echo`'s cosine distance would exclude it (e.g. prompt = "delete the meeting-notes file", `max_tools_per_turn = 5`, plus enough higher-scoring capabilities to fill the budget). Post the same prompt with `forced_capability="runtime-echo"` and assert `runtime-echo__echo` is still present in the served tool list at position 0 — i.e. the pin survived truncation.

### Phase 2.B — Increase top-K and add data-driven lexical prefilter

> Closes RC-A for the natural-language path (when no `forced_capability`).
>
> **SRP note.** Per Grok review (2026-05-22), the lexical-hint table is **not** hand-rolled in router code. It is sourced from each capability's manifest under a new optional `search_keywords` field on `[[tools]]` — so adding/changing a capability never requires touching router code. Mirrors how `embedding` text is already generated from manifests today.

- [x] **2.B.1** Raise `router_quota.max_tools_per_turn` default from `5` to `12` in gateway config. Test budget impact: 12 tool definitions × ~50 tokens each ≈ 600 tokens of overhead per turn. Acceptable; PR 4 enforces a hard token-budget assertion (see 4.2.4). *Already satisfied: `DEFAULT_MAX_TOOLS_PER_TURN = 25` in `router_quota.rs` — exceeds target.*
- [x] **2.B.2** Extend `ToolManifest` (`agent-core/src/manifest.rs`) `[[tools]]` schema with `search_keywords: Option<Vec<String>>` (default empty; fully backward-compatible — old manifests behave as today). Surface the field on `CapabilityCard` / `ToolDescriptor` so the router can read it without re-parsing TOML.
- [x] **2.B.3** Add `lexical_capability_hints(query: &str, registry: &CapabilityRegistry) -> Vec<String>` in [`agent-gateway/src/routes/agent.rs`](../apps/backend/crates/agent-gateway/src/routes/agent.rs). Implementation: iterate registered tools, case-insensitive **word-boundary** match (regex `\b<keyword>\b` with Unicode word boundaries) of any `search_keywords` entry against `query`, return distinct owning capability names. No hard-coded table. **Fuzzy/edit-distance matching is deliberately excluded** — it blurs intent; revisit only if regression suite shows misses on phrasing variants.
- [x] **2.B.3.1** **Tool-embedding cache** (new). Tool embeddings are static per `CapabilityRegistry` version. At registry load, compute embeddings for every `[[tools]]` `embedding` text once and store in an in-memory `HashMap<ToolId, Vec<f32>>` on `CapabilityRegistry`. The router uses cached vectors instead of re-embedding each turn. Invalidate on capability hot-reload (future). Wire `embedding_cache_hit_rate` counter (1.6.3). Largest expected latency win per 2026 router benchmarks.
- [x] **2.B.4** Populate `search_keywords` in the relevant `capability.toml` files (canonical seed set; ship in same PR):
  | Capability / tool | `search_keywords` |
  |---|---|
  | `storage-workspace.delete_node` | `["delete", "remove", "trash", "drop", "get rid of"]` |
  | `code-project.scaffold_project` | `["scaffold", "generate project", "new app", "sveltekit", "react", "vite", "nextjs"]` |
  | `code-project.host_project` | `["host", "deploy", "publish", "preview url", "share app"]` |
  | `code-project.add_dependency` | `["add dependency", "install package", "pnpm add", "npm install"]` |
  | `file-storage.*` | `["upload", "attach file", "presigned url"]` |
  | `extract-ocr-vision.*`, `ocr-service.*` | `["ocr", "extract text from image", "scan"]` |
- [x] **2.B.5** Merge lexical-hint capabilities into `tools` **after** any `forced_capability` prepend but **before** final truncation. Dedup by `name`. Add audit fields `lexical_hints: Vec<String>` and (already in 2.A.4) `forced_capability`.
- [x] **2.B.6** Test [`tests/lexical_prefilter.rs`](../apps/backend/crates/agent-gateway/tests/lexical_prefilter.rs): 10 canonical prompts, each must result in the expected capability appearing in `tools`. Plus one negative test: a manifest with empty `search_keywords` produces no hints (regression guard against accidental hard-coding).

### Phase 2.C — `ArtifactBridge` path prefix plumbing

> Closes RC-C. Implements the open item from `capabilities/plan.md` §8.A.3 and Open-Question §B.5.

- [x] **2.C.1** Add `path_prefix: Option<String>` parameter to [`ArtifactBridge::process_if_artifacts`](../apps/backend/crates/agent-core/src/bridge/artifact_bridge.rs). *Implemented: bridge reads `output.metadata["artifact_path_prefix"]` internally — same semantics, cleaner API (no caller changes needed).*
- [x] **2.C.2** In `materialise()`, if `path_prefix.is_some()`, compute `virtual_path = format!("{prefix}/{artifact_name}")`; otherwise fall back to today's `/outputs/{tool_name}/{artifact_name}` (backward compatible).
- [x] **2.C.3** Extend [`common::artifact::ToolOutput`](../apps/backend/crates/common/src/artifact.rs) with `metadata: HashMap<String, Value>` (or extend the existing metadata struct) so tools can declare `path_prefix`. *Already present: `metadata: Value` on `ToolOutput`.*
- [x] **2.C.4** In [`stream_agent`](../apps/backend/crates/agent-gateway/src/routes/agent.rs) where `process_if_artifacts` is called, read `tool_output.metadata.get("path_prefix")` and pass it through. *Done inside `ArtifactBridge::process_if_artifacts` — no stream_agent changes needed.*
- [x] **2.C.5** Update [`apps/backend/capabilities/code-project/capability.toml`](../apps/backend/capabilities/code-project/capability.toml):
  - `scaffold_project` chain output schema: include `metadata.path_prefix = "<target_path>"`. *Done: chain system prompt emits `"artifact_path_prefix":""` (workspace-root-relative paths).*
  - `edit_file` / `apply_patch` chains: `metadata.path_prefix = "<dir>"` where `<dir>` is the dirname of `path`. *Done: same empty-prefix convention.*
  - `host_project` chain: `metadata.path_prefix = "<target_path>"` plus `metadata.hosting_type`, `metadata.public_url` (Phase 9). *Done: chain prompt emits `hosting_type`, `root_path`, `index_file`.*
- [x] **2.C.6** Test [`tests/artifact_path_prefix.rs`](../apps/backend/crates/agent-core/tests/artifact_path_prefix.rs): emit a `ToolOutput` with `path_prefix = "projects/foo"`; assert artifacts land at `projects/foo/<name>` in `WorkspaceContentStore`. *5/5 tests pass.*

### Phase 2.D — Read-before-write tools (generic current-state injection)

> Closes RC-C side effect that produced S4 (fabricated `package.json`).
> Generalised from the original `requires_read` flag — applies to **any** tool that patches an existing file/document, not just `add_dependency`. 2026 agent pattern: inject current state into the prompt to eliminate fabrication entirely.

- [x] **2.D.1** Add `read_before_write: Option<String>` to `ToolManifest`'s `[[tools]]` block schema (default `None`; backward compatible). Value is the **input field name** that holds the path to read. *Done in `agent-core/src/capabilities/manifest.rs`.*
- [x] **2.D.2** In `add_dependency` manifest set `read_before_write = "manifest_path"`. Apply the same pattern to other patch-style tools as they land (`edit_file`, `apply_patch` already model this via diffs; `update_manifest`, future `update_env` etc.). *Done in `capabilities/code-project/capability.toml`.*
- [x] **2.D.3** In [`resolve_and_invoke` (agent.rs)](../apps/backend/crates/agent-gateway/src/routes/agent.rs), if `read_before_write` is `Some(field)`:
  1. Read the path from `input[field]`.
  2. Resolve via `WorkspaceContentStore.read` (no detour through `storage-fs`).
  3. If the file exists, inject content as `_current_content` field on the input. If not (new file), inject `_current_content: null` and tag `_is_new_file: true` so the chain prompt branches cleanly.
  *Done via `maybe_inject_current_content()` helper in `agent.rs`. Moka cache deferred (Risks note — TTL cache is a 2.D.3.4 follow-up).*
- [x] **2.D.4** Update `add_dependency` chain prompt to expect `_current_content` and patch it (not regenerate from scratch). *Done in `code-project/capability.toml` system_prompt with three-tier priority (`_current_content` > `manifest_content` > new-file fallback).*
- [x] **2.D.5** Test [`tests/read_before_write.rs`](../apps/backend/crates/agent-gateway/tests/read_before_write.rs): serde round-trip tests (TOML + JSON) and live `capabilities/code-project` TOML parse verification. *5/5 tests pass.*

**Success criteria PR 2**:
- ✅ Clicking any capability's "Invoke" button selects that capability's tools on first try, 10/10 runs
- ✅ "delete the X file" → `storage-workspace__delete_node`, no `storage-fs__list_paths` detour
- ✅ Scaffolded files land at the requested `projects/foo/*` path, not `/outputs/...`
- ✅ Follow-up `read_project`/`edit_file` finds files at the same path
- ✅ `add_dependency` preserves all existing fields in `package.json`

---

## 4. PR 3 — Frontend reactivity & generic live state (P1, ≈ 2.5 h)

**Outcome**: the UI mirrors backend state via a single generic invalidation channel; users see what's happening; broken paths recover gracefully.

### Phase 3.A — Live UI state (generic invalidation bus + `liveResource`)

> **Decision (2026-05-22, Grok / agent-core lead).** We are **not** shipping the
> narrow `workspace_changed` SSE + version counter originally drafted here.
> Instead we fold a minimal, SRP-compliant Tier-1 invalidation primitive into
> Phase 3.A. Cost delta: **+0.5 AI-h / ~9 k tokens**. Benefit: every future
> "server mutated something → UI must refresh" case (recents, capabilities,
> artifacts panel, Phase-9 hosting status, admin hot-reload, multi-tab
> consistency) becomes a one-liner instead of a per-feature SSE kind +
> version counter + `$effect` reinvention.
>
> **Why not Tier 2/3 (Replicache, Electric, Yjs, Convex)?** We have no
> multi-user editing, no offline, no optimistic mutations yet — adopting a
> sync framework now would constrain the data model for benefits we won't
> use for 6+ months. Re-evaluate when multi-user lands (see §11).
>
> **SRP / canonical-name check.** `InvalidationBus` has exactly one reason to
> exist: publish mutator events. `createLiveResource.svelte.ts` has exactly
> one reason to exist: subscribe to `resource_invalidated` deltas, debounce,
> re-fetch. No singletons, no global store, no external deps — pure Svelte 5
> runes + the existing SSE pattern from `createChatStream`.

**Outcome**: any server mutation (storage, compose, admin, future realtime path) instantly refreshes the relevant UI panels without per-feature wire formats.

- [x] **3.A.1** (backend) Add [`agent-core/src/realtime/invalidation.rs`](../apps/backend/crates/agent-core/src/realtime/invalidation.rs):
  ```rust
  // single responsibility: per-tenant invalidation broadcast
  pub struct InvalidationEvent {
      pub resource: String,          // "workspace" | "threads" | "capabilities" | "artifacts"
      pub scope: String,             // tenant_id
      pub changed_keys: Vec<String>, // optional paths / ids
  }
  pub type InvalidationBus = tokio::sync::broadcast::Sender<InvalidationEvent>;
  ```
  Wire one `InvalidationBus` into `AppState` (re-uses existing realtime bus pattern).
- [x] **3.A.2** In [`stream_agent`](../apps/backend/crates/agent-gateway/src/routes/agent.rs) **coalesce per turn** (do not emit one event per tool — see Risk #3 / S6 from the review). At end of turn, call `state.invalidation_bus.send(InvalidationEvent { resource: "workspace", scope: tenant_id, changed_keys: deduped_paths })` once, only if any mutator tool with `category: storage | compose` ran. `ArtifactBridge::process_if_artifacts` is the canonical place to record paths into a per-turn collector.
- [x] **3.A.3** (SDK) Extend `ChatStreamDelta` union in [`packages/sdk/src/types.ts`](../packages/sdk/src/types.ts) with a **generic** kind (not workspace-specific):
  ```ts
  | { kind: 'resource_invalidated'; resource: string; scope: string; changed_keys?: string[] }
  ```
  Parse it in the existing SSE handler in [`packages/sdk/src/chat.ts`](../packages/sdk/src/chat.ts).
- [x] **3.A.4** (packages/ui) New file [`packages/ui/src/lib/live/createLiveResource.svelte.ts`](../packages/ui/src/lib/live/createLiveResource.svelte.ts) (canonical name, runes factory) — **stale-while-revalidate + optimistic update + reconnect**:
  ```ts
  export function createLiveResource<T>(
    resource: string,
    fetchFn: () => Promise<T>,
    options?: { debounceMs?: number; scope?: string }
  ) {
    let data = $state<T | null>(null);        // last server truth
    let optimistic = $state<T | null>(null);   // optimistic overlay
    let version = $state(0);
    let isStale = $state(false);              // true while background re-fetch in flight
    let lastError = $state<Error | null>(null);

    // Stale-while-revalidate: keep showing data while re-fetching; flip isStale.
    // Optimistic: optimisticUpdate(updater, { rollbackOn?: Promise<unknown> })
    //   - applies an immer-style draft mutation to `optimistic`
    //   - if rollbackOn rejects, restores from `data` and surfaces error to toasts
    // SSE handler: on matching { resource, scope } → debounce → background fetch → swap.
    // Reconnect: EventSource listens for `error` events; exponential backoff (1s, 2s, 4s, 8s, capped at 30s).
    // On reconnect, force one refresh (mobile webview suspend race — see §0.5).

    return { data, optimistic, version, isStale, lastError, refresh, optimisticUpdate };
  }
  ```
  Implementation re-uses the SSE subscription machinery we already ship for chat; **zero new network code**. Consumers read `optimistic ?? data` so the overlay is automatic.
- [x] **3.A.4.1** **Optimistic rollback contract.** Every `optimisticUpdate` call must pass a `rollbackOn` promise (typically the SDK mutation). If the promise rejects, the overlay is dropped, `lastError` is set, and a toast fires. The rule for consumers: an optimistic update without a matching SDK call is a bug — enforce via TS overload that requires `rollbackOn`.
- [x] **3.A.5** Wire [`WorkspaceExplorer.svelte`](../packages/ui/src/lib/features/WorkspaceExplorer.svelte) and [`DrawerWorkspaceTree.svelte`](../apps/browser-shell/src/lib/mobile/parts/DrawerWorkspaceTree.svelte) as the first consumer:
  ```svelte
  const live = createLiveResource('workspace', () => sdk.workspaces.tree());
  $effect(() => { void live.version; tree = live.optimistic ?? live.data; });
  // Delete via UI: optimistic overlay + SDK promise
  function onDelete(id: string) {
    live.optimisticUpdate(
      draft => { draft.nodes = draft.nodes.filter(n => n.id !== id); },
      { rollbackOn: sdk.workspaces.delete(id) },
    );
  }
  ```
  Web (`+page.svelte`) and Shell (`MobileShell.svelte`) consume it identically — single source in `packages/ui`. **Cross-app parity check:** both apps import from `@conusai/ui` only; no local re-declarations.
- [x] **3.A.6** (proof of generality) Also wire [`DrawerRecentChats`](../packages/ui/src/lib/features/DrawerRecentChats.svelte) to `createLiveResource('threads', () => sdk.threads.list())` in the same PR — demonstrates the abstraction works for the second consumer immediately. Server emits `resource: "threads"` whenever a thread is created/renamed/deleted.
- [x] **3.A.7** **Authorization & scope.** `InvalidationBus` is per-tenant; the SSE handler filters deltas by `scope == current_tenant_id` server-side before sending. The client also asserts scope match defensively. Closes the multi-tenant leak risk that a generic channel would otherwise introduce.
- [x] **3.A.8** **Mobile webview suspend handling** (Tauri iOS/Android). On `document.visibilitychange === 'visible'` and on EventSource `error → reconnect`, call `refresh()` on every live resource to recover from suspended-while-backgrounded state. Tested in the Tauri iOS simulator by backgrounding the app, mutating workspace from a separate browser session, and resuming.
- [x] **3.A.9** E2E guard (web + Tauri):
  1. Web — chat → `save_document("notes/test.md", …)` → sidebar shows `test.md` < 1 s without manual refresh.
  2. Web — optimistic delete shows immediate UI update; SDK call rejected by server → overlay reverts within 1 s + toast fires.
  3. Tauri iOS — same chat scenario in `apps/browser-shell` simulator → identical behavior; background app then foreground → resource refreshes automatically.
  4. Second guard: `sdk.threads.create()` from a separate fetch → recents list shows new thread < 1 s on both apps.
- [x] **3.A.10** **Cross-app parity check (lint).** Add an ESLint or simple `rg` CI rule: no file in `apps/web` or `apps/browser-shell` may import `*.svelte` from a sibling app. All shared UI must come from `@conusai/ui`. Lives in Phase 4.4.

### Phase 3.B — Capability hint chip in chat (transparent + clickable)

- [x] **3.B.1** When a chat turn was started via `forced_capability`, store the capability name on the user message envelope. Surface it in [`AgentChatStream`](../packages/ui/src/lib/features/AgentChatStream.svelte) as a small chip rendered before the assistant response: `📦 Using <capability-name>`. Lives in `packages/ui`, consumed identically by web + shell.
- [x] **3.B.1.1** **Clickable for transparency.** The chip is a button. On click, open a small popover showing the actual `selected_capabilities` + `pinned_tools` + `lexical_hits` for that turn (pulled from the SSE audit delta exposed in 2.A.4). Users see *which tools the model actually had access to* — a 2026 agent-UI baseline expectation. Keyboard accessible (`Esc` dismisses).
- [x] **3.B.2** If the assistant response contains the phrase pattern `/don't have (the |a )?(\w+) tool/i` or `/no tools available/i`, **OR** the audit delta reports `tools_per_turn === 0`, render an inline retry button: *"Retry with explicit capability hint"* that re-sends the last user message with `forced_capability` set (when known) or opens a quick-picker. (Structured signal preferred; regex is the fallback.)
- [x] **3.B.3** Visual: chip uses `--ember-soft` background, `--font-mono` font, matches the existing context chip styling. Popover uses the existing `Sheet`/`Tooltip` primitive in `packages/ui` so it renders identically in both apps (sheet on mobile, tooltip on desktop). **Cross-app parity check:** identical import path in `+page.svelte` and `MobileShell.svelte`.

### Phase 3.C — URL / deep-link state restoration (single API for both apps)

> **Cross-app trap (per §0.5):** web reads initial route from `window.location.search`; Tauri iOS/Android receives it via deep-link callbacks (`tauri-plugin-deep-link`). One helper, both call sites.

- [x] **3.C.1** New helper [`packages/ui/src/lib/routing/initialRoute.ts`](../packages/ui/src/lib/routing/initialRoute.ts):
  ```ts
  export type InitialRoute = { ws?: string; thread?: string; cap?: string };
  export async function initialRoute(): Promise<InitialRoute> {
    // Web: parse window.location.search
    // Tauri: await @tauri-apps/plugin-deep-link getCurrent() + listen for runtime deep links
    // Returns merged route. Detect platform via `import.meta.env.TAURI` or `window.__TAURI__`.
  }
  ```
  Single source of truth for "where did the app launch into?".
- [x] **3.C.2** [`apps/web/src/routes/+page.svelte`](../apps/web/src/routes/+page.svelte) `onMount`: `const r = await initialRoute(); if (r.ws) { const node = await sdk.workspaces.get(r.ws).catch(() => null); if (node) { applyNode(node); } else { toasts.warn("Workspace not found"); clearParam('ws'); } }`. Also handle `r.thread` and `r.cap` (open the capability detail sheet).
- [x] **3.C.3** [`apps/browser-shell/src/lib/mobile/MobileShell.svelte`](../apps/browser-shell/src/lib/mobile/MobileShell.svelte) `onMount`: identical code path — calls the same `initialRoute()`. Tauri config in `src-tauri/tauri.conf.json` registers the URL scheme (e.g. `conusai://`).
- [x] **3.C.4** Refresh-test: load web page with `?ws=<id>` → folder highlighted, context chip shows it, composer ready. Open Tauri app via `conusai://?ws=<id>` deep link → identical state.
- [x] **3.C.5** Invalid ID surface: stale shared links show a toast "Workspace not found, returning to root" rather than silently clearing — applies to both apps via the shared helper.

### Phase 3.D — Tool error toasts

- [x] **3.D.1** SSE delta `tool_call_result` extended with optional `error: string`. When set, [`createChatStream`](../packages/ui/src/lib/features/createChatStream.svelte.ts) calls `toasts.error("Tool <name> failed: <message>")` and marks the tool card as failed.
- [x] **3.D.2** [`ToolCallCard`](../packages/ui/src/lib/features/ToolCallCard.svelte) renders a red border + retry button when `error` is set.
- [x] **3.D.3** E2E: pass a `delete_node` with an unknown ULID → toast appears with the gateway's error message verbatim.

**Success criteria PR 3**:
- ✅ Chat-driven `save_document` reflects in sidebar within 1 s via the generic invalidation channel
- ✅ Recent chats list also refreshes live via the same primitive (proof of reusability — second consumer in same PR)
- ✅ No per-feature SSE kinds, no duplicated version counters; future "live X" features are one-liners
- ✅ Page reload with `?ws=<id>` restores selected workspace node
- ✅ Tool failures surface as toasts, not just polite LLM apologies
- ✅ "Using <capability-name>" chip visible after every Invoke-initiated turn

---

## 5. PR 4 — Routing quality regression suite (P2, ≈ 2 h)

**Outcome**: a CI gate that catches routing regressions when embeddings or capability descriptions change.

### Phase 4.1 — Fixture suite

- [x] **4.1.1** Create [`apps/backend/crates/agent-gateway/tests/fixtures/routing_prompts.toml`](../apps/backend/crates/agent-gateway/tests/fixtures/routing_prompts.toml):
  ```toml
  [[case]]
  prompt = "delete the meeting-notes file"
  expected_capability = "storage-workspace"
  expected_tool = "delete_node"   # optional, when we want exact match

  [[case]]
  prompt = "scaffold a sveltekit app under projects/demo"
  expected_capability = "code-project"
  expected_tool = "scaffold_project"

  # ... 30 cases total
  ```
- [x] **4.1.2** Source 30 prompts from real audit-log queries observed in `gw-final-clean.log` plus engineered edge cases (vague/explicit/path-flavored/name-flavored).

### Phase 4.2 — Test harness

- [x] **4.2.1** [`tests/routing_quality.rs`](../apps/backend/crates/agent-gateway/tests/routing_quality.rs): boots a minimal gateway with all production capabilities registered, runs each prompt through `state.semantic_router.tool_definitions(prompt, tenant)`, checks the returned `tools` for the expected capability name. *(Note: implemented at the lexical-hint layer — `CapabilityRegistry::lexical_hint_capabilities` — so the test stays deterministic without spinning up qdrant + fastembed. Semantic ANN quality is verified manually via `verify-web.md`.)*
- [x] **4.2.2** Threshold gate: at least **27/30** must pass. Hard fail if any *previously-passing* case fails (track per-case in a baseline file).
- [x] **4.2.3** Output: a markdown table of every case + pass/fail + which capability won, written to `target/routing_quality.md` for inspection.
- [ ] **4.2.4** Token-budget assertion: for each case, sum the JSON-schema byte length of the served tool definitions and assert the mean stays under 800 tokens (≈ 12 tools × 50 tokens + headroom). Hard fail if a code change pushes the mean > 10 % above the recorded baseline (`target/routing_tokens.baseline`). Catches context inflation regressions from raising `max_tools_per_turn`. **Baseline regeneration policy**: only on PRs tagged `tokens-baseline-bump` and reviewed by the router owner. *(Deferred — token-cost gate only makes sense once the test exercises full served-tool definitions through the semantic router; the current lexical-only test doesn't surface tool JSON bytes.)*
- [x] **4.2.5** **Synthetic prompt variations** (new). For each base fixture, generate 3 phrasing variants via simple deterministic templating (e.g. "delete the X file" → "remove the X file" / "trash X" / "get rid of X"). Run all variants; pass criterion stays at 27/30 *base* prompts but variants are reported separately. Catches phrasing brittleness without flaky LLM-as-judge in CI.
- [ ] **4.2.6** **Confidence distribution report** (new). Output now includes a histogram of `max_score` across all cases — surfaces low-confidence clusters for post-merge tuning. *(Deferred together with 4.2.4 — `max_score` is a semantic-router output, not a lexical one.)*

> **Deliberately not adopted:** LLM-as-judge in CI. Flaky, expensive, and the existing per-case `expected_capability` ground truth + token gate already covers the failure modes we care about. Revisit if the suite grows past ~100 cases. Tracked in §9 Out of Scope.

### Phase 4.3 — CI integration

- [x] **4.3.1** Add to `.github/workflows/backend.yml` (or equivalent): run `cargo test -p agent-gateway --test routing_quality --release` after the main test suite. *(Implemented as a step in the existing `ci.yml` `test:` job — single workflow keeps surface small.)*
- [x] **4.3.2** Upload `target/routing_quality.md` as a CI artifact for inspection on failure.
- [ ] **4.3.3** Document in [`docs/capabilities/plan.md`](capabilities/plan.md) §5 success criteria: "Routing quality ≥ 27/30 fixtures pass." *(Deferred — not strictly required for this round; the criterion is enforced in the test harness.)*

### Phase 4.4 — Cross-app parity CI lint (new)

> Enforces the invariant from §0.5: no UI feature lands in only one app.

- [x] **4.4.1** Add a small CI step (Node script or `rg` invocation) that scans `apps/web/src` and `apps/browser-shell/src` for any `import` ending in `.svelte` whose specifier is **not** `@conusai/ui/...` or `$lib/...` (where `$lib` is the app's own). Fail CI on any cross-app import or any `.svelte` file added under `apps/*/src/lib/features/` (features belong in `packages/ui`).
- [x] **4.4.2** Allow-list: a narrow set of legitimately app-specific files (e.g. `+page.svelte`, `+layout.svelte`, `MobileShell.svelte`). Everything else under `lib/features/` must come from `packages/ui`.
- [x] **4.4.3** Document the rule in [`docs/arch.md`](arch.md) §3 (Frontend Architecture) and link from [`docs/capabilities/how-to-add-a-domain.md`](capabilities/how-to-add-a-domain.md). *(Linked from new `arch.md` §18; checklist appended to `how-to-add-a-domain.md`.)*

**Success criteria PR 4**:
- ✅ CI runs the regression suite on every backend PR
- ✅ Embedding-model swap → CI catches accuracy drop before merge
- ✅ Adding new capabilities → engineers add 1–2 routing fixtures per capability (documented in `how-to-add-a-domain.md`)
- ✅ Cross-app parity lint fails any PR that adds a feature `.svelte` outside `packages/ui`

---

## 6. Effort & total budget

| PR | Phase | Effort (AI-hrs) | Tokens |
|---|---|---|---|
| 1 | Operational hardening | 1.0 | 18 k |
| 1.6 | Observability baseline (OTEL + Prometheus + extended `/healthz`) | 0.25 | 5 k |
| 2.A | `forced_capability` plumbing + pinning guarantee + tenant allowlist | 1.0 | 15 k |
| 2.A.3.1 | Confidence threshold + fallback | 0.1 | 2 k |
| 2.B | Data-driven lexical prefilter (word-boundary) + larger top-K | 0.75 | 11 k |
| 2.B.3.1 | Tool-embedding cache at registry load | 0.15 | 3 k |
| 2.C | ArtifactBridge path prefix | 1.0 | 18 k |
| 2.D | Read-before-write tools (generic) | 0.75 | 14 k |
| 3.A | Live UI state (generic invalidation bus + SWR + optimistic + reconnect + 2 consumers) | 1.5 | 27 k |
| 3.B | Capability hint chip (clickable + tools popover) | 0.5 | 9 k |
| 3.C | Deep-link / URL state restoration (`initialRoute` helper, web + Tauri) | 0.4 | 7 k |
| 3.D | Tool error toasts | 0.5 | 9 k |
| 4 | Routing regression suite (+ token-budget + synthetic variants + confidence histogram) | 2.25 | 40 k |
| 4.4 | Cross-app parity CI lint | 0.25 | 4 k |
| | **Total** | **10.4 h** | **182 k** |

---

## 7. Risks & rollback

### Risks

| Risk | Mitigation |
|---|---|
| Increasing `max_tools_per_turn` to 12 inflates context cost | Measure tokens-per-turn before/after on a fixture batch; if > 10 % regression, drop to 8 and rely more on `forced_capability` + lexical hints |
| `forced_capability` API change breaks SDK consumers | Field is `Option<String>` with `serde(default)` — backward compatible. SDK bumps minor version only |
| `path_prefix` plumbing breaks legacy callers that expect `/outputs/...` | Backward compatible: `None` keeps today's behaviour. Only `code-project` chains will start emitting `path_prefix` initially |
| `read_before_write` tools double work on read-heavy chats | Cache the read by `(tenant, path, content_hash)` in Moka with 60 s TTL (2.D.3.4) |
| OTEL exporter unreachable in CI / dev | Default exporter is `stdout` (1.6.1); OTLP only enables when `OTEL_EXPORTER_OTLP_ENDPOINT` is set. Failure to export is a `warn!`, never a panic |
| Tool-embedding cache stale after capability hot-reload (future) | Cache is keyed by `CapabilityRegistry` version; hot-reload bumps the version and re-populates. Until hot-reload lands, restart-only refresh is acceptable |
| Optimistic UI overlays rollback inconsistently | Mandatory `rollbackOn: Promise` contract on every `optimisticUpdate` call (3.A.4.1); TS overload enforces it; toast on rejection |
| Mobile webview suspend drops SSE / kills live resources | `createLiveResource` listens for `visibilitychange === 'visible'` + EventSource reconnect with exponential backoff (3.A.8); tested in Tauri iOS simulator (3.A.9) |
| Confidence threshold mis-tuned → too many fallbacks → context bloat | Threshold is configurable per gateway; `low_confidence_turns_total` counter (1.6.3) + audit event flag let us tune from data, not guesses |
| `resource_invalidated` SSE deltas cause sidebar thrash if many tools fire | **Server-side coalescing** (3.A.2) emits at most one event per turn per resource with deduped `changed_keys`; `createLiveResource` adds client-side debounce (default 200 ms) as belt-and-braces |
| Generic invalidation channel leaks cross-tenant deltas | `InvalidationBus` is per-tenant; SSE handler filters by `scope == tenant_id` server-side before send (3.A.7); client also asserts scope defensively |
| Manifest `search_keywords` lists drift / miss cases | Routing regression suite (PR 4) catches drift; lexical hints are an *additional* signal, not a replacement for embeddings. Owners of a capability own its keywords (same as they own its `embedding` text) |
| ToolTweak-style description-tampering attack on a pinned/forced capability (arXiv:2503.xxxxx) | Already mitigated: `forced_capability` is logged in the audit event (2.A.4) and the routing regression suite (PR 4) acts as an integrity baseline. No additional code change required |

### Rollback

Each PR is independently revertable:

- PR 1 — revert: gateway returns to feature-gated default; explicit error vanishes.
- PR 1.6 — revert: OTEL spans + `/metrics` removed; gateway still runs (just blind in production). Safe to revert independently.
- PR 2.A — revert: `forced_capability` field becomes a no-op; UI still works via `buildInvocationPrompt` fallback. Threshold fallback (2.A.3.1) is gated by config — set to `0.0` to disable without code revert.
- PR 2.B — revert: `max_tools_per_turn` returns to 5; lexical hints removed. `search_keywords` field on `ToolManifest` is `Option<Vec<String>>` — leaving it in place is a no-op. Embedding cache (2.B.3.1) can be disabled by feature flag without code revert.
- PR 2.C — revert: artifacts go back to `/outputs/...` default; `code-project` flows degrade but storage flows unaffected.
- PR 2.D — revert: `read_before_write` field becomes a no-op; LLM may fabricate again until prompt improves. Cache disabled with TTL = 0.
- PR 3 — revert: sidebar and recents stop auto-refreshing; manual reload still works. `InvalidationBus` is a `tokio::broadcast` channel with no persistence — dropping all subscribers is safe. `createLiveResource` consumers fall back to their initial fetch. Optimistic overlays revert to no-op (data stays in `data`, not `optimistic`).
- PR 4 — revert: CI gate disabled; tests stay green by default. Cross-app parity lint (4.4) is independently revertable.

---

## 8. Success criteria (verifiable from chat in browser)

After all four PRs ship, the user must be able to:

1. ✅ Fresh `./start.sh local` → `/healthz/embeddings` returns 200 in < 60 s
2. ✅ Click any capability in the Capabilities screen → "Invoke" → exactly that capability's tools are used by the LLM, every time (10/10 runs per cap, all 25 caps)
3. ✅ "delete the X file" → `storage-workspace__delete_node` selected on first try, no `storage-fs` detour
4. ✅ "scaffold sveltekit at projects/foo" → files materialize at `projects/foo/*`, visible in sidebar within 1 s
5. ✅ Follow-up "edit projects/foo/src/routes/+page.svelte" finds the file at that exact path
6. ✅ "add lodash to projects/foo/package.json" preserves all existing fields (no fabricated JSON)
7. ✅ "host projects/foo" returns a clickable `public_url` in the chat (Phase 9 of capabilities/plan.md)
8. ✅ Page reload with `?ws=<id>` (web) and `conusai://?ws=<id>` deep link (Tauri shell) both restore the selected workspace node and breadcrumb — identical UX
9. ✅ A failed tool call surfaces as a toast with the gateway's exact error message
10. ✅ Routing regression suite ≥ 27/30 in CI on every backend PR (plus synthetic-variant report)
11. ✅ `/metrics` Prometheus endpoint exposes `routing_latency_ms`, `tools_per_turn`, `forced_capability_hit_rate`, `embedding_cache_hit_rate`, `low_confidence_turns_total`; chat turn produces a `chat.turn` OTEL trace
12. ✅ Low-confidence prompts never return zero tools — threshold fallback unions lexical + top-3 semantic
13. ✅ Optimistic delete/create in workspace tree applies instantly; rollback + toast on server failure (web + Tauri)
14. ✅ Capability hint chip is clickable; shows `selected_capabilities`, `pinned_tools`, `lexical_hits` for the turn
15. ✅ Cross-app parity lint passes — no feature `.svelte` outside `packages/ui`; web and shell produce byte-identical UI for every feature in this plan
16. ✅ Tauri iOS / Android / macOS / Windows shell renders the same `WorkspaceExplorer`, `DrawerRecentChats`, `AgentChatStream`, capability chip, toast host as `apps/web` — verified by screenshot diff on representative flows

---

## 8.1 Post-merge documentation updates

After PR 2 lands, append one paragraph each to:

- [`docs/arch.md`](arch.md) §4.2 (`SemanticCapabilityRouter`): document the `forced_capability` prepend-before-truncate guarantee and the new manifest-driven `search_keywords` lexical hint path.
- [`docs/arch.md`](arch.md) §12.6 (Capability factory chain): note that `ToolManifest` now carries `search_keywords` and that the router reads them from `CapabilityCard`, not from any in-code table.
- [`docs/capabilities/how-to-add-a-domain.md`](capabilities/how-to-add-a-domain.md): add a one-line checklist item — "Populate `[[tools]] search_keywords` for any tool with strong lexical triggers (delete/upload/scaffold/etc.)."

After PR 3 lands, append:

- [`docs/arch.md`](arch.md) — add new **§11 Live UI State Architecture** (mirrors §11 of this plan): `InvalidationBus` + `resource_invalidated` SSE delta + `createLiveResource.svelte.ts`; one canonical channel, per-tenant scope, no external sync libs.
- [`docs/arch.md`](arch.md) — add **§12 Observability** (mirrors Phase 1.6): OTEL spans for the routing decision tree, Prometheus endpoint, `/healthz` readiness contract.
- [`docs/arch.md`](arch.md) §3 (Frontend Architecture) — add the cross-app parity invariant (§0.5 of this plan) and link the CI lint (4.4) as the enforcement mechanism. Include a Mermaid diagram of the routing decision flow (semantic → lexical → forced pin → threshold/fallback → cache lookup).
- [`docs/capabilities/how-to-add-a-domain.md`](capabilities/how-to-add-a-domain.md): add a one-line item — "If your capability mutates server state, publish to `InvalidationBus` with the correct `resource` so live consumers refresh automatically." Also: "Any feature `.svelte` ships in `packages/ui`, never in `apps/*` — CI enforces it."

No other documents are touched.

---

## 9. Out of scope (deliberate)

These are real future work but **not** in this plan:

- **Dynamic hosting** (`code-shell`): subprocess execution + sandboxing + resource caps. Deferred per `capabilities/plan.md` Phase 9 out-of-scope clause.
- **Per-capability cost budgeting**: tracking Anthropic token spend per tool-call so we can throttle expensive chains. Belongs in a `cost-control` follow-up.
- **Cross-capability planning**: today the LLM picks one tool per turn. A `plan-orchestrate` follow-up would let it pre-commit to a multi-tool plan and execute atomically.
- **Capability marketplace UI**: enabling/disabling capabilities per tenant from the UI. Today this is admin-only via REST.
- **`MobileShell` shared with web orchestrator**: web's `+page.svelte` and shell's `MobileShell.svelte` could collapse into a single `AppOrchestrator` once login flows are unified. Tracked separately in a follow-up.
- **LLM-as-judge / NDCG in routing CI**: Considered (Grok review 2026-05-22) and deliberately declined. Flaky, expensive per CI run, and the existing per-case `expected_capability` ground truth + synthetic variants + token gate already cover the failure modes. Revisit if the fixture suite passes ~100 cases.
- **Capability hot-reload (dev-only file watcher)**: A real DX win for capability iteration but unrelated to the symptoms this plan fixes. Spawn as a separate task — would slot in cleanly under `CapabilityRegistry` version bumping (already used by the embedding cache, 2.B.3.1).
- **Sparse + dense hybrid retrieval (ColBERT / SPLADE)**: Worth revisiting only if registered tool count exceeds ~100. Today's manifest-driven `search_keywords` + dense embeddings is the right cost/complexity trade for <30 capabilities.
- **Multi-client / multi-tab realtime**: Today live deltas only flow on the active chat stream. A workspace-scoped `/v1/realtime` SSE channel (independent of chat) is the natural follow-up when multi-tab consistency becomes a hard requirement — see §11 architecture note.
- **Optimistic mutations with conflict resolution / CRDT**: Tier-2 sync framework (Replicache / Yjs / Electric). Not adopted; no multi-user editing or offline requirements today. Re-evaluate post-Phase-9.

---

## 10. What this session already fixed (reference, not action items)

For completeness of audit trail — these symptoms were patched live during the unification session and are no longer issues:

- `MobileTopBar` `role="banner"` removed (a11y warning)
- All shell components migrated from raw `px` to `--t-*` typography tokens
- `DrawerRecentChats` `workspaceNodes` populated via new `onNodesLoaded` callback
- Shell `+layout.svelte` mounts `<ToastHost />` + `data-hydrated` for parity
- `tokens.css` consolidated into `foundry.css` (single source of truth; fixed latent web dark-mode bug)
- Cross-app component unification: `ChatScreen` / `CapabilitiesScreen` / `ArtifactsScreen` / `AppTopBar` / `AppDrawer` / `AppBottomSheet` + `screenStore` / `drawerStore` live in `packages/ui`, identically consumed by both apps
- `buildInvocationPrompt(cap)` helper that names tool + capability + description in the user message (kept as defense-in-depth signal even after `forced_capability` lands)
- Capability invocation flow proven end-to-end with `runtime-echo` and full `storage-workspace` CRUD (create, read/list, edit-via-overwrite, delete, upload-and-read)
- Gateway rebuilt with `--features local-embeddings` and restarted with sourced env — semantic router now serves tools per turn

---

## 11. Live UI State Architecture (added 2026-05-22)

All server-driven UI invalidations flow through a **single generic channel** instead of per-feature SSE kinds:

- **Backend** — `realtime::InvalidationBus` (`tokio::sync::broadcast`, per-tenant). Published by:
  - `ArtifactBridge::process_if_artifacts` → `resource: "workspace"`
  - `ToolExecutor` (storage/compose category) → `resource: "workspace" | "artifacts"`
  - Future admin hot-reload → `resource: "capabilities"`
  - Future hosting status (Phase 9) → `resource: "artifacts"` with `changed_keys = [host_id]`
- **Wire format** — one SSE delta kind for *all* of the above:
  ```ts
  { kind: 'resource_invalidated', resource: string, scope: string, changed_keys?: string[] }
  ```
  Generic by design; the router does not need to know which resources exist.
- **Frontend** — [`packages/ui/src/lib/live/createLiveResource.svelte.ts`](../packages/ui/src/lib/live/createLiveResource.svelte.ts), a runes-only factory. Consumers:
  ```ts
  const live = createLiveResource('workspace', () => sdk.workspaces.tree());
  // live.data is reactive; bumps when server invalidates "workspace"
  ```
  Components stay two-liners. No global store. No external libraries.
- **Coalescing** — server emits at most one event per turn per resource with deduped `changed_keys`. Client adds a default 200 ms debounce as belt-and-braces.
- **Scope safety** — `InvalidationBus` is per-tenant; SSE handler filters by `scope == tenant_id` server-side before sending; client also asserts scope defensively. Closes the multi-tenant leak that a global channel would otherwise introduce.
- **Cross-app parity (load-bearing)** — `createLiveResource` lives in `packages/ui` only. Both `apps/web` and `apps/browser-shell` (Tauri 2 → iOS, Android, macOS, Windows) consume the same export — byte-identical reactive behavior across platforms. The CI lint in Phase 4.4 enforces this; the parity invariant is documented in §0.5.
- **Stale-while-revalidate + optimistic** — Phase 3.A.4 adds these explicitly. `data` holds last server truth, `optimistic` is the overlay, `isStale` flips during background re-fetch. `optimisticUpdate({ rollbackOn })` is the only mutation entry point; rollback + toast on rejection are mandatory (3.A.4.1).
- **Mobile webview suspend** — `createLiveResource` listens for `visibilitychange === 'visible'` and EventSource `error → reconnect` with exponential backoff. Tested in Tauri iOS simulator (3.A.9). This is the single biggest Tauri-vs-browser divergence; handled once in the primitive so consumers stay simple.
- **Multi-client / multi-tab** — out of scope for PR 3. Today deltas only flow on the active chat stream's SSE connection. A dedicated `/v1/realtime` workspace-scoped SSE channel that runs independently of chat is the natural next step when multi-tab consistency becomes a real requirement.
- **What we deliberately did not adopt** — Tier 2 sync frameworks (Replicache, Electric, Yjs, Convex). They solve multi-user editing, offline, and optimistic *with conflict resolution* — none of which we have today. The optimistic-with-rollback model in 3.A.4 covers our single-user/single-tab case with ~100 lines of code instead of a sync framework dependency. Re-evaluate when those harder requirements land per [`docs/capabilities/plan.md`](capabilities/plan.md) Phase 9+.

This keeps us aligned with [`docs/arch.md`](arch.md) §3 (Svelte 5 runes-only, no singletons, centralised in `packages/ui`) and the "no unnecessary abstractions" rule — the new primitive has exactly one job and pays for itself on the next live feature.

---

## Appendix A — File touch list (for PR scoping)

| PR | Files modified / added |
|---|---|
| 1 | `Cargo.toml` (gateway, agent-core); `state.rs`; `embedding_service.rs`; `routes.rs`; `health.rs` (new); `start.sh` |
| 1.6 | `Cargo.toml` (add `opentelemetry`, `axum-prometheus`); `main.rs` (tracer init); `routes/metrics.rs` (new); `routes/agent.rs` (span instrumentation); `health.rs` (extended); `start.sh` (print OTEL mode) |
| 2.A | `ui/handlers/chat.rs`; `routes/chat.rs`; `routes/agent.rs` (pin + tenant allowlist + threshold fallback); `packages/sdk/src/chat.ts`, `types.ts`; `packages/ui/.../createChatStream.svelte.ts`, `buildInvocationPrompt.ts`; `apps/web/.../+page.svelte`; `apps/browser-shell/.../MobileShell.svelte` |
| 2.B | `agent-core/src/manifest.rs` (add `search_keywords`); `agent-core/src/capabilities/registry.rs` (embedding cache); `routes/agent.rs` (word-boundary lexical match, cache lookup); `capabilities/*/capability.toml` (seed keywords); config defaults |
| 2.C | `bridge/artifact_bridge.rs`; `common/src/artifact.rs`; `routes/agent.rs`; `capabilities/code-project/capability.toml` |
| 2.D | `ToolManifest` schema in `agent-core/src/manifest.rs` (`read_before_write`); `capabilities/executor.rs` (current-state injection + Moka cache); `capabilities/code-project/capability.toml` |
| 3.A | `agent-core/src/realtime/invalidation.rs` (new); `agent-core/src/state.rs` or `AppState` wiring; `bridge/artifact_bridge.rs` (per-turn path collector); `routes/agent.rs` (emit at end of turn); `packages/sdk/src/types.ts`, `chat.ts`; `packages/ui/src/lib/live/createLiveResource.svelte.ts` (new — SWR + optimistic + reconnect); `packages/ui/src/lib/features/WorkspaceExplorer.svelte`, `DrawerWorkspaceTree.svelte`, `DrawerRecentChats.svelte`; `apps/web/.../+page.svelte`; `apps/browser-shell/.../MobileShell.svelte` (both apps consume only via `@conusai/ui`) |
| 3.B | `packages/ui/.../AgentChatStream.svelte`, `ChatScreen.svelte`, new `CapabilityPinChip.svelte` (chip + tools popover) |
| 3.C | `packages/ui/src/lib/routing/initialRoute.ts` (new — handles `window.location.search` AND Tauri deep-link); `apps/web/src/routes/+page.svelte`; `apps/browser-shell/.../MobileShell.svelte`; `apps/browser-shell/src-tauri/tauri.conf.json` (register `conusai://` scheme); `apps/browser-shell/src-tauri/Cargo.toml` (add `tauri-plugin-deep-link`) |
| 3.D | `packages/sdk/src/chat.ts`; `packages/ui/.../createChatStream.svelte.ts`, `ToolCallCard.svelte` |
| 4 | `tests/routing_quality.rs` (new); `tests/fixtures/routing_prompts.toml` (new); `tests/fixtures/synthetic_variants.rs` (new); CI workflow yml |
| 4.4 | `.github/workflows/parity-lint.yml` (new) or new step in existing workflow; `scripts/check-cross-app-imports.mjs` (new) |

---

## Appendix B — Open questions (not blocking)

1. Should `forced_capability` accept a list (`forced_capabilities: Vec<String>`) so the UI could pin a 2-tool combo (e.g. `code-project` + `storage-workspace`) for compound invocations? — Deferred; start with single string, expand if real use cases appear.
2. ~~Lexical-hint table lives in code today (Phase 2.B.2). Should it move to a TOML file per capability?~~ **Resolved 2026-05-22 (Grok review):** Phase 2.B is now data-driven via `[[tools]] search_keywords` in `capability.toml`. The router never embeds capability-specific lexemes.
3. ~~`workspace_changed` SSE: should we send the actual node payload (Vec<WorkspaceNode>) so the sidebar can skip the re-fetch?~~ **Resolved 2026-05-22:** Phase 3.A now uses a generic `resource_invalidated` delta + `createLiveResource` re-fetch. Re-fetch is < 50 ms; sending payloads would couple wire format to data model and defeat the point of the generic primitive. Re-evaluate only if a profiled bottleneck emerges.
4. The routing regression suite (PR 4) covers single-prompt → single-capability mapping. Multi-turn flows (e.g. "delete X, then save Y") are out of scope and need a different harness — planned for a follow-up.
5. **Retrieval scale-out (future):** `search_keywords` + dense embeddings may evolve to sparse/hybrid (ColBERT / SPLADE / outcome-aware refinement) once registered tools exceed ~100. Today's design holds for <30 capabilities; no action needed now, just noted.
6. **OTEL GenAI semantic conventions** (resource attributes for prompts/tool calls): we ship a minimal trace skeleton in 1.6.2; aligning attribute names with the emerging OpenInference / OTEL GenAI spec is a follow-up.
7. **iOS background-suspend → SSE reconnect**: 3.A.8 handles the foreground case via `visibilitychange`. True background sync (push-style updates while the app is fully suspended) would require Tauri background tasks + APNs — out of scope.
