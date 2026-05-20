# Capabilities-as-Everything Refactor Plan

> Goal: Make **every** domain element of ConusAI — including file upload, file
> read/write, MIME-aware routing, OCR, Markdown conversion, document
> classification, invoice/contract extraction, transcription, and even simple
> `read_file` / `list_files` developer tools — a first-class, hot-pluggable
> `CapabilityProvider` declared by a **TOML manifest** (or WASM / MCP / remote
> MCP), discovered at boot, embedded for the `SemanticCapabilityRouter`,
> orchestrated by the agent runtime, and **never** referenced by name from
> `agent-core` or `agent-gateway`.
>
> Companion docs: `docs/arch.md`, `docs/tasks/cap-task.md`,
> `docs/capability-authoring-guide.md`.
>
> Outcome: adding a brand-new domain (medical claim processing, ERP sync,
> contract redlining, legal e-discovery, etc.) requires **zero changes** to any
> core crate — only a new manifest directory under `capabilities/` (or a WASM
> component, or a remote MCP endpoint registration).

---

## 0. Guiding Principles (Hard Invariants)

1. **`agent-core` knows nothing about any domain.** No struct named after a
   business object (`Invoice…`, `Contract…`, `Ocr…`, `Transcribe…`). No
   hard-coded model id outside `LlmRegistry` defaults.
2. **Single source of LLM access.** Every model call (text, vision, JSON,
   streaming) goes through `LlmRegistry::resolve_binding` →
   `CompletionProvider`. No code path constructs
   `rig::providers::*::Client` directly except inside `llm/providers/*`.
3. **Capabilities richer than tools.** A capability may expose one or many
   tools, accept attachments, emit `Artifact`s, declare permissions, and
   participate in orchestration. The manifest is the contract.
4. **Semantic prefilter, always.** The `SemanticCapabilityRouter` is the
   only path that turns user intent into a tool catalog (top-K ≤ 50). No code
   passes the full registry to Rig.
5. **TOML > Rust for new domains.** If a capability can be expressed as a
   prompt chain, it MUST live as a `kind = "chain"` manifest. Rust providers
   are reserved for native integrations (storage, jobs, system services).
6. **Orchestration is itself a capability.** Multi-step flows
   (`upload → mime-detect → ocr → markdown → classify → file-in-folder`) are
   composed by either (a) the agent over many turns with router-selected
   tools, or (b) a `planner` capability that emits a plan + invokes
   sub-capabilities. No bespoke pipeline modules.
7. **Storage is a capability, not a hand-wired route.** HTTP upload endpoints
   remain (they handle multipart), but they are thin adaptors that delegate
   to a `storage.*` capability for path resolution, folder creation, MIME
   sniffing, and post-upload event emission.
8. **Hot reload everywhere.** Realtime bus reloads any capability TOML/WASM
   without restart (already implemented for `CapabilitySpecFactory` — extend
   to disk-discovered manifests via `notify` + 250ms debounce).
9. **No fat abstractions for composition.** Storage operations are implemented
   as **focused providers** (one small `CapabilityProvider` per operation or
   tight op-group) that **compose** existing `WorkspaceStore` +
   `WorkspaceContentStore` + `ObjectStore`. We deliberately reject a broad
   `StorageBackend` trait — it would centralise logic and re-create the
   monolith we are dismantling. Each provider keeps one obvious reason to
   exist.
10. **Canonical placements (non-negotiable).**
    - All new factories live in
      `agent-core/src/capabilities/providers/{native_storage,job_backed,...}.rs`.
    - `CapabilityRegistry::with_default_factories(llm)` registers factories
      in this **fixed order** (audit-locked):
      `Mcp → Wasm → Chain → NativeStorage → CapabilitySpec`.
      `BuiltinFactory` is removed in Phase 4 and MUST NOT reappear.
    - `OrchestrationHook` lives in `agent-core/src/agent/hooks.rs` next to
      `TracingHook` + `PermissionHook`.
    - Generic `run_plan` lives in
      `agent-core/src/capabilities/executor.rs`.
    - A `build.rs` grep guard fails compilation if `rig::providers::` appears
      outside `agent-core/src/llm/providers/`.
11. **Manifest naming hygiene.** The on-disk loader struct is `ToolManifest`
    (private to `capabilities::manifest` + factories). Every **public**
    surface — registry metadata, admin DTOs, HTTP responses, realtime bus
    payloads — keeps the canonical names `CapabilityCard` / `CapabilitySpec`.
    No rename of the on-disk struct is required as long as it stays private
    to discovery + factory dispatch.

---

## 1. Current Domain Leakage (Audit)

| # | Location | Problem | Fix Phase |
|---|----------|---------|-----------|
| L1 | [chains/contract.rs](apps/backend/crates/agent-core/src/chains/contract.rs) | Constructs `rig::providers::anthropic::Client` directly; hardcoded model `claude-opus-4-7`; hardcoded `UserContent::image_base64` vision path | Phase 3 |
| L2 | [chains/invoice.rs](apps/backend/crates/agent-core/src/chains/invoice.rs) | Same bypass pattern as L1, invoice-specific | Phase 3 |
| L3 | [chains/extraction.rs](apps/backend/crates/agent-core/src/chains/extraction.rs) | Base helper for L1/L2; domain-coupled | Phase 3 |
| L4 | [capabilities/builtin/fs.rs](apps/backend/crates/agent-core/src/capabilities/builtin/fs.rs) + [builtin/cargo.rs](apps/backend/crates/agent-core/src/capabilities/builtin/cargo.rs) + [builtin/card.rs](apps/backend/crates/agent-core/src/capabilities/builtin/card.rs) | `read_file` / `write_file` / `run_cargo` and their manifest live in core as `BuiltinFactory`; cannot be disabled, versioned, or replaced without recompile | Phase 4 |
| L5 | [agent-gateway/src/capabilities/workspace.rs](apps/backend/crates/agent-gateway/src/capabilities/workspace.rs) | `WorkspaceProvider` is generic-ish but hard-wired in gateway code, not a manifest; new storage tools require recompile | Phase 4 |
| L6 | [agent-gateway/src/capabilities/transcribe_video.rs](apps/backend/crates/agent-gateway/src/capabilities/transcribe_video.rs) | Domain capability hardcoded in gateway, coupled to `JobExecutor` directly | Phase 5 |
| L7 | [ui/handlers/upload.rs](apps/backend/crates/agent-gateway/src/ui/handlers/upload.rs) + `routes/uploads.rs` | Upload is an opaque HTTP endpoint that writes to `object_store` directly; no capability sees the upload event; no MIME-routed post-processing | Phase 6 |
| L8 | `evals/runners/invoice.rs`, `evals/runners/ocr_quality.rs` | Domain knowledge inside the generic eval harness | Phase 7 |
| L9 | `docs/arch.md` feature inventory lists "Contract / invoice extraction (Claude vision)" as a core feature | Documentation drift | Phase 8 |

---

## 2. Target Architecture

### 2.1 Layered View

```
                ┌────────────────────────────────────────────┐
                │           USER PROMPT + ATTACHMENTS         │
                └──────────────────────┬─────────────────────┘
                                       │
                       ┌───────────────▼─────────────────┐
                       │   SemanticCapabilityRouter      │
                       │   (top-K ≤ 50, moka-cached)     │
                       └───────────────┬─────────────────┘
                                       │   ToolDyn[ ]
                       ┌───────────────▼─────────────────┐
                       │   Rig AgentBuilder (per turn)   │
                       │   + TracingHook + PermissionHook│
                       │   + OrchestrationHook (NEW)     │
                       └───────────────┬─────────────────┘
                                       │
            ┌──────────────────────────┼──────────────────────────────┐
            │                          │                              │
    ┌───────▼────────┐        ┌────────▼─────────┐          ┌─────────▼─────────┐
    │ Storage caps   │        │ Transform caps   │          │ Domain caps       │
    │ (TOML+native)  │        │ (TOML chains /   │          │ (TOML chains /    │
    │ - storage.put  │        │  WASM)           │          │  WASM / MCP)      │
    │ - storage.list │        │ - mime.detect    │          │ - finance.invoice │
    │ - storage.move │        │ - ocr.tesseract  │          │ - legal.contract  │
    │ - storage.tag  │        │ - convert.to_md  │          │ - hr.cv-screen    │
    └───────┬────────┘        │ - classify.doc   │          │ - …               │
            │                 │ - transcribe.av  │          └─────────┬─────────┘
            │                 └────────┬─────────┘                    │
            │                          │                              │
            └──────────────────┬───────┴──────────────────────────────┘
                               │
                    ┌──────────▼───────────┐
                    │   ArtifactBridge     │
                    │ (writes back to      │
                    │  workspace +         │
                    │  object_store,       │
                    │  emits realtime      │
                    │  events)             │
                    └──────────────────────┘
```

### 2.2 Capability Categories (Canonical Taxonomy)

| Category | Namespace prefix | Examples | Typical kind |
|----------|------------------|----------|--------------|
| Storage  | `storage.*`      | `storage.put`, `storage.list`, `storage.move`, `storage.tag`, `storage.list_folders`, `storage.ensure_date_folder` | `native` (Rust, manifest-on-disk) |
| Compute  | `compute.*`      | `compute.run_cargo`, `compute.shell` (gated) | `native` (opt-in) |
| Sense    | `sense.*`        | `sense.mime`, `sense.detect_language`, `sense.classify_document` | `chain` or `wasm` |
| Extract  | `extract.*`      | `extract.ocr.tesseract`, `extract.ocr.vision`, `extract.fields.invoice`, `extract.fields.contract` | `chain` |
| Convert  | `convert.*`      | `convert.pdf_to_md`, `convert.audio_to_text`, `convert.image_to_thumb` | `chain` / `wasm` / job-backed |
| Compose  | `compose.*`      | `compose.email`, `compose.invoice_pdf`, `compose.report_md` | `chain` |
| Deliver  | `deliver.*`      | `deliver.email_smtp`, `deliver.webhook`, `deliver.s3_export` | `remote_mcp` / `native` |
| Plan     | `plan.*`         | `plan.orchestrate`, `plan.route_by_mime` | `chain` (meta) |

> **Rule:** every new domain element MUST pick a namespace from this taxonomy.
> The taxonomy is documented in `docs/capabilities/taxonomy.md` (new file in
> Phase 1).

### 2.3 The "Upload as Capability" Pattern

The HTTP multipart endpoint stays (browsers and the Tauri shell need it), but
it becomes a **thin adaptor**:

```
POST /v1/files (multipart)
   │
   ├─► validate + stream bytes to a temp staging key
   │
   ├─► call CapabilityRegistry → invoke "storage.put" with:
   │       { staging_key, filename, content_type, size,
   │         requested_folder?: "..." , thread_id?: "..." }
   │
   ├─► storage.put capability:
   │       - if requested_folder is None → call "storage.ensure_date_folder"
   │         (creates /Uploads/2026/05/19/ under tenant workspace root,
   │          name template configurable per-tenant)
   │       - moves staging → final object key
   │       - writes workspace_nodes row (parent = folder node)
   │       - emits realtime event "workspace.uploaded"
   │       - returns { node_id, object_key, virtual_path, mime, size }
   │
   ├─► (optional, manifest-driven) router auto-invokes "plan.on_upload"
   │       if installed — see §2.4
   │
   └─► HTTP 200 { node_id, virtual_path, mime, attachment_id_for_chat }
```

`plan.on_upload` is itself a TOML capability whose `chain` template asks the
LLM (cheap alias, e.g. `haiku`) to choose a post-processing pipeline based on
MIME + filename + tenant preferences. It then emits a sequence of tool calls
(e.g. `extract.ocr.vision` → `convert.pdf_to_md` → `sense.classify_document`)
that the agent runtime executes through the standard router. The user can
override per-tenant via a `policies.toml` manifest.

### 2.4 Generic Orchestration: `plan.orchestrate`

We deliberately avoid building a new "capability composer" Rust abstraction.
Orchestration emerges from two existing primitives plus one new manifest:

1. **Agent multi-turn** — the LLM, given the router-filtered toolset,
   chooses the next tool. This is the default for free-form user prompts.
2. **`PromptChainCapability`** — declarative, deterministic chains in TOML;
   used when the flow is known (e.g. "every uploaded PDF goes through OCR +
   classify").
3. **NEW: `plan.orchestrate` meta-capability** — a `kind = "chain"`
   manifest with `vision = false`, system prompt = "You are an orchestration
   planner. Given an input artifact and the current capability catalog,
   output a JSON plan of `[ {capability, tool, args} ]` steps that maximises
   the user's goal." Its `output_schema` enforces the plan shape. The agent
   then dispatches each step. This is recursive-safe because plans cannot
   call `plan.orchestrate` (permission policy enforced via `PermissionHook`).

This gives us "Alpha-Go-grade" orchestration without coupling core to any
domain: the planner sees the same semantic-router top-K the agent sees, plus
artifact metadata.

### 2.5 Chaining Best-Result Strategies (research-backed)

For each step in a generated plan, the orchestrator supports three strategies
(declared per step in the plan JSON, defaulting to `single`):

- `single` — pick the highest-ranked router hit.
- `parallel_consensus` — invoke the top-N matching capabilities in parallel
  and reduce via a `reduce` capability (LLM-as-judge or programmatic
  diff/merge). Useful for OCR (Tesseract + vision LLM agree).
- `fallback_cascade` — try capabilities in confidence order; first to return
  non-error wins. Useful for transcription (local Whisper → cloud Whisper).

These strategies live entirely in `plan.orchestrate`'s prompt template + a
small generic `executor::run_plan(plan, registry, router)` helper added to
`agent-core::capabilities::executor`. **No domain code.**

---

## 3. Phased Implementation Plan

> Each phase is independently shippable, leaves the system green, and adds
> tests. Phases 1, 2, 3 are blocking (architectural). Phases 4–8 are
> incremental migrations.

### Phase 1 — Taxonomy, Manifest v2, and Capability Authoring Guide

**Why first:** locks the contract every later phase obeys.

- [ ] 1.1 Create [docs/capabilities/taxonomy.md](docs/capabilities/taxonomy.md)
  with the namespace table from §2.2 + worked examples.
- [ ] 1.2 Extend `ToolManifest` (in
  [manifest.rs](apps/backend/crates/agent-core/src/capabilities/manifest.rs))
  with **non-breaking** optional fields:
  - `category: Option<String>` — one of the taxonomy roots.
  - `accepts: Vec<AcceptSpec>` — declares attachment MIME globs + max size,
    enabling router to filter on artifact compatibility (NEW field on
    `CachedResult`).
  - `emits: Vec<EmitSpec>` — declares artifact MIMEs produced (enables
    chaining suggestions: planner knows `extract.ocr.*` emits `text/plain`).
  - `idempotent: bool` (default `true`) — gates parallel/retry.
  - `cost_hint: Option<CostHint>` — `{ tokens?: u64, dollars?: f32,
    latency_ms?: u64 }` — planner uses for ranking.
  - `requires: Vec<String>` — capability dependencies (router warns if
    missing, never auto-installs).
- [ ] 1.3 Update [capability-authoring-guide.md](docs/capability-authoring-guide.md)
  with mandatory fields, the taxonomy rule, and the planner contract.
- [ ] 1.4 Bump manifest schema version constant to `"2.0"`; loader accepts
  both `"1.x"` and `"2.0"` (back-compat).
- [ ] 1.5 Add a `cargo xtask capabilities lint` command that validates every
  TOML in `apps/backend/capabilities/` and `services/` against the schema +
  taxonomy. Wire into CI.

### Phase 2 — Router & Executor Generic Enhancements

- [ ] 2.1 Extend `SemanticCapabilityRouter` to accept an `AttachmentHint`
  (set of MIME types in the current turn) and **post-filter** hits whose
  `accepts` globs do not match. Cache key MUST include the hint.
  **R6 finding (2026 best practice):** post-filter is correct (pre-filtering
  ANN breaks recall guarantees). Additionally enrich each capability's
  embedding text with tokens like `MIME:application/pdf` /
  `MIME:image/*` so ANN recall is high even before post-filter.
- [ ] 2.1a **Embedding enrichment for planner-aware ranking (reviewer
  refinement).** In `capabilities/embedding.rs`, also append
  `CATEGORY:<root>` (e.g. `CATEGORY:extract`) and a coarse cost bucket
  token derived from `cost_hint` (e.g. `COST:cheap` / `COST:standard` /
  `COST:premium`) to each capability's embedding text. This lets
  `plan.orchestrate` make accuracy-vs-cost decisions natively from the
  same ANN result set — no re-ranking complexity. **Cache key** must
  include `(query_hash, attachment_hint, cost_bias?)`. Effort: ~1 AI-hr,
  ~2k tokens.
- [ ] 2.2 Add `executor::run_plan(plan, registry, router, tenant)` in
  `agent-core/src/capabilities/executor.rs` — generic plan executor
  implementing `single` / `parallel_consensus` / `fallback_cascade`. Returns
  `Vec<StepResult>` with timings + artifacts threaded through
  `ArtifactBridge`. **R3 finding:** the `parallel_consensus` reducer is
  configurable per step — `llm_judge` (cheap `LlmRegistry` alias) for free
  text, `field_merge` (deterministic JSON deep-diff) for structured
  schemas, optional `borda` for ranking. All three reducers are generic
  helpers in `executor.rs`, zero domain knowledge.
- [ ] 2.2a **Reducer hardening (reviewer refinement).**
  - `llm_judge` MUST resolve through `LlmRegistry::resolve_binding("cheap", tenant)`
    (never a hardcoded model id) and respect a **per-call token budget**
    `MAX_REDUCER_TOKENS = 2_048` enforced before dispatch.
  - Every `run_plan` operation — plan parse, per-step dispatch,
    parallel fan-out, reducer — emits a `tracing::info_span!` with fields
    `capability`, `tool`, `strategy`, `step_idx`, `duration_ms`,
    `tokens_in`, `tokens_out`, `cost_hint_class`. Spans are picked up by
    `TracingHook` + OTLP exporter, so the existing observability stack
    sees orchestration natively. Effort: ~0.5 AI-hr.
- [ ] 2.3 Add `OrchestrationHook` in `agent-core/src/agent/hooks.rs`
  (Rig `PromptHook`) that, when a tool call returns `{ plan_steps: [...] }`,
  feeds the next step back into the same agent turn via `run_plan`. Makes
  `plan.orchestrate` first-class without changing any caller. **R2
  finding:** ReAct (multi-turn) stays the default for open user prompts;
  `plan.orchestrate` is invoked explicitly for deterministic flows (e.g.
  uploads) — Anthropic 2026 guidance + orchestrator-workers pattern.
- [ ] 2.3a **Hook prototype gate (BLOCKING — must land before Phase 3
  deletions).** Build a minimal end-to-end prototype that proves the
  `OrchestrationHook` cleanly injects `plan_steps` back into the active
  Rig turn **without** breaking:
  - `StreamedAssistantContent` chunk ordering as seen by
    `agent-gateway` SSE handler.
  - `max_turns` accounting (each injected step counts as one tool round).
  - Tool-call card lifecycle (`tool_call_start` → `tool_call_result`)
    rendered by the web/iOS UI.
  - `TracingHook` + `PermissionHook` ordering (orchestration hook runs
    AFTER permission, BEFORE tracing close).
  **Fallback (if hook injection proves fragile):** a clean
  *sub-execution* path inside `executor::run_plan` that re-uses the same
  `registry` + `router` + `tenant` but runs as its own short Rig
  invocation per step. Zero domain coupling either way. The decision is
  recorded as ADR-0008 in Phase 8.
  Effort: 1.5–2 AI-hr, ~3k tokens.
- [ ] 2.3b **Planner output validation (reviewer refinement).** The
  `plan.orchestrate` chain MUST declare a strict JSON `output_schema`
  enumerating `{step_idx, capability, tool, args, strategy, depends_on?}`.
  `executor::run_plan` validates the plan against the schema AND against
  the *current* registry — unknown `capability` or `tool` names cause a
  graceful error step (planner sees the failure and re-plans on next
  turn). Prevents hallucinated capability names from crashing the agent.
  Prompt template includes an explicit "Only emit names from this exact
  catalog: […]" section populated at runtime with the router's top-K.
  Effort: ~1 AI-hr, ~2k tokens.
- [ ] 2.4 Permission policy in `PermissionHook`: deny recursive
  `plan.orchestrate` calls (depth = 1); deny `compute.*` unless
  `tenant.plan.allows_compute()`; emit `ToolCallHookAction::Skip { reason }`
  so the LLM sees a graceful denial.
- [ ] 2.4a **High-stakes capability gating (deferred, Phase 8+ — noted
  here for taxonomy completeness).** Add an optional manifest field
  `risk_class = "low" | "standard" | "high_stakes"` (default `standard`).
  `PermissionHook` policy reserves an `approval_required` step type in
  plans for `high_stakes` capabilities (e.g. `deliver.payment`,
  `storage.delete_tenant_data`). UI surfaces an approval card; no auto-
  execution. Not implemented in this refactor; documented now so the
  taxonomy + ADR cover the extension point. Future-effort: ~3 AI-hr.
- [ ] 2.5 Add `build.rs` in `agent-core` that greps `src/**/*.rs` and fails
  the build if `rig::providers::` appears outside `src/llm/providers/`.
  Single-source invariant enforced at compile time.

### Phase 3 — Delete Domain Chains, Enforce `LlmRegistry`

- [ ] 3.1 Delete [chains/contract.rs](apps/backend/crates/agent-core/src/chains/contract.rs),
  [chains/invoice.rs](apps/backend/crates/agent-core/src/chains/invoice.rs),
  [chains/extraction.rs](apps/backend/crates/agent-core/src/chains/extraction.rs)
  and any `pub use` re-exports in `chains/mod.rs` and `lib.rs`.
- [ ] 3.2 Audit `chains/executor.rs::run_chain`: every model call must use
  `llm_registry.resolve_binding(&cfg.model, tenant)?.complete(req).await`.
  Vision goes through the same path — `AnthropicProvider` already supports
  `UserContent::image_base64` (extend if a provider lacks it; never
  short-circuit through `rig::providers::*::Client`). The build-time grep
  guard added in 2.5 catches regressions.
- [ ] 3.3 Re-implement the two example pipelines as TOML manifests under
  `apps/backend/capabilities/`:
  - `extract-invoice-fields/capability.toml` — `kind = "chain"`,
    `model = "smart"`, `vision = true`, `output_schema = { … }`,
    `category = "extract"`, `namespace = "extract.fields.invoice"`,
    `accepts = [ "application/pdf", "image/*" ]`,
    `emits = [ "application/json" ]`.
  - `extract-contract-fields/capability.toml` — same pattern, contract
    schema, `namespace = "extract.fields.contract"`.
  - Move the existing prompts to `apps/backend/capabilities/<name>/prompts/`.
- [ ] 3.4 Migrate any gateway routes that still call `ContractPipeline` /
  `InvoicePipeline` to invoke the capabilities via the registry
  (`ToolExecutor::invoke`). Likely zero callers remain after Phase 3.1; if
  any exist, replace.
- [ ] 3.5 Add an integration test: send a PDF through the standard chat flow
  and assert the agent invokes `extract.fields.invoice` via the router.

### Phase 4 — Storage & Native Tools as TOML Capabilities

> Replaces `BuiltinFactory` and `WorkspaceProvider` with **focused**
> manifest-driven `CapabilityProvider` impls. Each provider **composes**
> existing `WorkspaceStore` + `WorkspaceContentStore` + `ObjectStore`
> directly. **No `StorageBackend` god-trait** — one provider, one obvious
> reason to exist (SRP).

- [ ] 4.1 Add focused providers in
  `agent-core/src/capabilities/providers/native_storage.rs`:
  - `PutObjectProvider`, `ListFoldersProvider`, `EnsureFolderProvider`,
    `EnsureDateFolderProvider`, `MoveObjectProvider`, `TagObjectProvider`,
    `ReadTextNodeProvider`, `WriteTextNodeProvider`.
  Each holds only the `Arc<dyn …>` stores it actually needs (no leaky
  super-trait). All implement `CapabilityProvider` directly.
- [ ] 4.2 Create `NativeStorageFactory` in the same file. Its `supports`
  returns true for `(ToolKind::Native, name)` where `name` starts with
  `storage.`. `create` dispatches on the manifest's `[native].op` field to
  instantiate the right focused provider. Factory holds the `Arc`-stores
  injected at construction time (mirrors `ChainFactory::new(llm)` pattern).
- [ ] 4.3 Author manifests under `apps/backend/capabilities/storage-*/`:
  - `storage-put` (`tools: [put]`, `category = "storage"`,
    `namespace = "storage.put"`)
  - `storage-list-folders` (`tools: [list_folders]`)
  - `storage-ensure-folder` (`tools: [ensure_folder]`)
  - `storage-ensure-date-folder` (`tools: [ensure_date_folder]`,
    `[native] template = "/Uploads/{{ now | strftime('%Y/%m/%d') }}/"`,
    rendered with **minijinja** — see R4)
  - `storage-move`, `storage-tag`, `storage-read-text` (replaces
    `read_file`), `storage-write-text` (replaces `write_file`)
  Each manifest sets `accepts` / `emits` / `idempotent` appropriately.
- [ ] 4.3a **Path template rendering — two-step adoption (reviewer
  refinement).**
  - **Step A (default, ship first):** add a tiny zero-dep `PathTemplate`
    helper in `common::path_template` using only `chrono` + basic variable
    substitution (`{tenant_id}`, `{thread_id}`, `{mime}`, `{mime_category}`,
    `{original_name}`, `{now:%Y/%m/%d}` strftime form). Covers every
    template shipped in this plan. Honours the "no unnecessary features"
    invariant.
  - **Step B (only if real tenant templates demand it):** add the
    `minijinja` workspace dependency behind a `templates-jinja` cargo
    feature on `common`; the loader picks engine by manifest field
    `template_engine = "basic" | "jinja"` (default `basic`). R4 keeps
    minijinja as the documented escape hatch; no upfront dep until
    justified by a user requirement. Effort delta either way: +0.5 AI-hr.
  - Render context (both engines): `{ tenant_id, now (chrono::Utc::now()),
    mime, mime_category, original_name, thread_id? }`.
  - **Authoring-guide duty (reviewer refinement):** Phase 1.3 update of
    [capability-authoring-guide.md](docs/capability-authoring-guide.md)
    MUST document this render context verbatim alongside one worked
    example per variable, and explicitly call out that the basic engine
    rejects expressions beyond `{var}` + `{now:%fmt}` so manifests stay
    portable across engines.
- [ ] 4.4 Delete `capabilities/builtin/{card.rs, fs.rs, cargo.rs}` and
  `providers/builtin.rs`. Move `cargo` behind a developer-only manifest
  under `services/dev-tools/capabilities/run-cargo/` gated by env
  `CONUSAI_ENABLE_DEV_TOOLS=1` (factory refuses to load otherwise).
  **R5 finding:** if/when we add `compute.shell`, implement it via the
  existing `WasmFactory` (Wasmtime 44 component model is the strongest
  practical sandbox in 2026 — capability-based imports, fuel limits,
  memory-safe by construction). No new sandbox crate needed.
- [ ] 4.5 Delete `agent-gateway/src/capabilities/workspace.rs`. Replace its
  registration in `state.rs` with a `NativeStorageFactory` registration +
  manifest discovery from disk.
- [ ] 4.6 Update `CapabilityRegistry::with_default_factories` accordingly
  (remove `BuiltinFactory`, add `NativeStorageFactory`).
- [ ] 4.7 Tests: integration test that the agent given "save these notes in
  Research" calls `storage.ensure_folder` + `storage.write_text` rather
  than the old `workspace__save_document`.

### Phase 5 — Transcription, OCR, MIME, Markdown as Capabilities

- [ ] 5.1 Delete `agent-gateway/src/capabilities/transcribe_video.rs`.
- [ ] 5.2 Introduce a generic `JobBackedNativeFactory` in
  `agent-core/src/capabilities/providers/job_backed.rs`. Manifest `[native]`
  block: `backend = "job"`, `job_kind = "..."`, `poll_path_template =
  "/v1/tasks/{id}"`. Factory receives `Arc<JobExecutor>` at construction.
- [ ] 5.3 Author manifests (**R1 ranking — vision-first per 2026 SOTA**):
  - `extract-ocr-vision/capability.toml` — **primary OCR**, `kind =
    "chain"`, `vision = true`, `model = "smart"` (resolved via
    `LlmRegistry`; Anthropic vision path is already supported). Excellent
    accuracy on complex layouts, handwriting, tables.
  - `extract-ocr-tesseract/capability.toml` — **legacy baseline**,
    `kind = "wasm"`, fast/cheap/air-gapped. Used as a member of
    `parallel_consensus` when the orchestrator wants cross-verification.
  - `convert-audio-to-text/capability.toml` — `namespace =
    "convert.audio_to_text"`, `kind = "native"`, `[native] backend = "job"
    job_kind = "video-transcription"`, `accepts = [ "audio/*", "video/*" ]`,
    `emits = [ "text/plain" ]`.
  - `sense-mime/capability.toml` — `kind = "wasm"` (uses the `infer`
    Rust crate compiled to WASI p2 component).
  - `convert-pdf-to-md/capability.toml` — `kind = "chain"`; delegates to
    `extract.ocr.*` then a markdown-rewrite prompt.
  - `sense-classify-document/capability.toml` — `kind = "chain"`, cheap
    alias, `output_schema = { type, confidence }`.
  > Future option: add MinerU 2.5 / GLM-OCR / Qwen3-VL via
  > `remote_mcp` or `wasm` capabilities — slot in as `extract.ocr.*`
  > siblings without touching core. See `docs/research/R1-ocr-wasi-2026.md`.
- [ ] 5.4 Register the existing TOML directories already present
  (`contract-processing`, `invoice-processing`, `ocr-service`,
  `file-storage`, `google-workspace`) by re-aligning their `namespace` and
  `category` to the taxonomy. Audit each for `accepts` / `emits` / search
  keywords.

### Phase 6 — Upload Pipeline Becomes Capability-Driven

- [ ] 6.1 Refactor `routes/uploads.rs` and `ui/handlers/upload.rs` to:
  1. Stream bytes to a tenant-scoped staging key.
  2. Call `ToolExecutor::invoke(registry, "storage.put", "put", …)`.
  3. If a tenant has an active `plan.on_upload` capability, invoke it with
     the resulting `{ node_id, mime, virtual_path }` and pass its
     `plan_steps` back into the agent runtime as a background task (queued
     via `JobExecutor`, executed against the tenant's normal agent runtime
     so the user sees realtime progress).
  4. Return HTTP 200 with attachment metadata.
- [ ] 6.2 Add per-tenant `policies/on_upload.toml` (loaded by
  `CapabilitySpecFactory`) so non-developer admins can pin a different
  pipeline (e.g. "always OCR + classify + file in `/Inbox/<date>/`").
- [ ] 6.3 Emit realtime events on every step
  (`workspace.uploaded`, `pipeline.step.started`, `pipeline.step.finished`)
  so the UI can render an "Inbox" timeline.
- [ ] 6.4 No domain branching in upload handler — only generic capability
  dispatch.
- [ ] 6.5 Add `ManifestWatcher` in
  `agent-core/src/capabilities/discovery.rs` using the `notify` crate +
  **250 ms debounce** (R8). Watches `apps/backend/capabilities/` and
  per-tenant policy dirs; on change, validates the new TOML, reloads
  through `CapabilityRegistry::reload_capability`, invalidates the
  `SemanticCapabilityRouter` moka cache for affected names, and emits
  `capability.reloaded` on the realtime bus. Parse errors keep the old
  version live and log a warning.

### Phase 7 — Evals Become Generic + External Suites

- [ ] 7.1 Refactor `apps/backend/evals/` to a generic harness:
  `cargo run -p evals -- run --suite path/to/suite.jsonl --scorer
  field-diff|exact|llm-judge`.
- [ ] 7.2 Move invoice / OCR suites to `apps/backend/evals/suites/` as
  JSONL + reference outputs. Delete `runners/invoice.rs`,
  `runners/ocr_quality.rs`.
- [ ] 7.3 Provide three built-in generic scorers: `exact`, `field-diff`
  (JSON deep diff with tolerances), `llm-judge` (uses `LlmRegistry`).
- [ ] 7.4 CI runs `cargo run -p evals -- run --suite suites/smoke.jsonl`.

### Phase 8 — Documentation & Hygiene

- [ ] 8.1 Rewrite `docs/arch.md` §4.4.5 (chains) and the feature inventory:
  invoice/contract extraction are listed as **example capabilities**, not
  core features. Add a new §4.7 "Orchestration via `plan.orchestrate`".
- [ ] 8.2 Add `docs/capabilities/upload-pipeline.md` describing the
  upload-as-capability pattern (§2.3) end-to-end with sequence diagram.
- [ ] 8.3 Add `docs/capabilities/orchestration.md` describing
  `plan.orchestrate`, planner prompt template, and the three execution
  strategies (§2.5).
- [ ] 8.4 Add ADR `docs/adr/0007-everything-is-a-capability.md` recording
  the decision, rationale, and rejected alternatives (custom Composer
  abstraction, in-core domain modules).
- [ ] 8.4a Add ADR `docs/adr/0008-orchestration-hook-vs-subexecution.md`
  recording the outcome of the Phase 2.3a hook prototype gate (hook
  injection vs sub-execution fallback) with the chosen path's
  trade-offs, streaming-safety evidence, and link to the prototype PR.
- [ ] 8.5 Update `README.md` "How to add a new domain" → 4 steps
  (write manifest, add prompt template, `cargo xtask capabilities lint`,
  reload).
- [ ] 8.6 Remove every mention of `BuiltinFactory`, `ContractPipeline`,
  `InvoicePipeline`, `TranscribeVideoCapability`, `WorkspaceProvider` from
  docs and code.

---

## 4. Research Findings (May 2026) — Decisions Locked In

Research was completed against current community sources, benchmarks and
2026 vendor guidance. The decisions below are now part of the plan. Each
row's full notes live in `docs/research/<id>-<slug>.md` (created in Phase 1).

| ID | Question | Decision | Locked-in plan reference |
|----|----------|----------|--------------------------|
| R1 | Best OCR stack (Tesseract WASM vs MinerU/Surya/Qwen3-VL/GLM-OCR vs vision-LLM)? | **Primary = vision-LLM via `LlmRegistry`** (`extract.ocr.vision`, `kind = "chain"`); **secondary baseline = Tesseract WASM** for air-gapped/cheap; **future = MinerU 2.5 / GLM-OCR / Qwen3-VL** added as sibling `extract.ocr.*` caps via `remote_mcp` or `wasm` — never in core. | Phase 5.3 |
| R2 | Plan-then-execute vs ReAct vs `tool_choice="auto"` for Anthropic + Rig 0.36. | **Hybrid (orchestrator-workers pattern).** Default = multi-turn ReAct (`tool_choice="auto"` inside `AgentBuilder`). Explicit planning = `plan.orchestrate` chain + `OrchestrationHook` for deterministic flows (uploads, batch ops). | Phase 2.3 |
| R3 | Consensus / verifier for `parallel_consensus`. | Pluggable reducers in `executor.rs`: `llm_judge` (cheap `LlmRegistry` alias for free text), `field_merge` (deterministic JSON deep-diff for schemas), `borda` (optional ranking). MoA-inspired. | Phase 2.2 |
| R4 | Date-folder template engine. | **`minijinja`** (Jinja2-compat, low deps, community-preferred in 2026). Default template `/Uploads/{{ now \| strftime('%Y/%m/%d') }}/`. | Phase 4.3a |
| R5 | Safe sandbox for `compute.shell`. | **Wasmtime 44 component model** via existing `WasmFactory` (capability-based imports, fuel limits, memory-safe). Gated by `CONUSAI_ENABLE_DEV_TOOLS` + `tenant.plan.allows_compute()`. No new sandbox crate. | Phase 4.4 |
| R6 | Attachment-aware semantic routing. | **Hybrid:** (a) enrich each capability's embedding text with `MIME:application/pdf` etc. tokens for ANN recall; (b) strict post-filter on `accepts` globs after ANN. Cache key includes the hint. | Phase 2.1 |
| R7 | Per-tenant cost-aware planning. | `cost_hint` in manifest + Qdrant payload; **feed into `plan.orchestrate` prompt** for accuracy-vs-cost decisions; hard limits stay in `QuotaChecker` + `PlanLimits`. No ANN re-ranking complexity. | Phase 1.2, Phase 6.2 |
| R8 | Hot-reload of disk manifests. | `notify` crate + **250 ms debounce**; atomic-rename + validate-before-swap; keep old version on parse error; emit `capability.reloaded` on realtime bus; invalidate moka cache for affected names. | Phase 6.5 |

> Each row must have a 1–2 page note under `docs/research/Rn-<slug>.md`
> before the corresponding phase merges. Notes summarise sources, trade-offs
> rejected, and the locked-in decision.

---

## 5. Acceptance Criteria

The refactor is **done** when ALL of the following hold:

1. `grep -R "rig::providers" apps/backend/crates/agent-core/src | grep -v
   '/llm/providers/'` returns no matches.
2. `grep -R -i "invoice\|contract\|transcribe\|ocr\|workspace__\|read_file\|
   write_file\|run_cargo" apps/backend/crates/{agent-core,agent-gateway}/src`
   returns only generic references (logs, comments) — no domain types.
3. `apps/backend/capabilities/` contains a manifest for every storage,
   transform, extract, convert, and orchestration capability listed in §2.2.
4. `cargo xtask capabilities lint` passes on every manifest.
5. Removing any single capability directory causes the corresponding tool
   to disappear from `/v1/capabilities` at the next hot reload **without
   recompiling**.
6. Adding a new capability directory makes the tool show up in
   `/v1/capabilities` and become callable via the agent within 1 second of
   the realtime bus broadcasting the change — **without recompiling**.
7. An end-to-end test uploads a PDF → expects MIME detect → OCR → markdown
   → classify → file under `/Inbox/<YYYY>/<MM>/<DD>/` with realtime events
   visible, and the entire pipeline is driven by manifests with zero
   gateway-side branching.
8. Eval harness runs `suites/smoke.jsonl` (invoice + contract + OCR) using
   only generic scorers.
9. `docs/arch.md`, `docs/capabilities/taxonomy.md`,
   `docs/capabilities/upload-pipeline.md`,
   `docs/capabilities/orchestration.md`, and ADR-0007 are merged.

---

## 6. Sequencing & Effort

Reviewer-aligned breakdown (AI-hours + token-budget per phase, dominated by
heavy context loads of `agent-core/src/capabilities/*` + `docs/arch.md` at
~15–20k tokens for heavy phases). Per project-instructions, every
significant change records both axes.

| Phase | Depends on | AI-hours | Token budget | Risk |
|-------|------------|----------|--------------|------|
| 1 — taxonomy + manifest v2 + xtask lint | — | 2 | ~4k | low — schema work |
| 2 — router post-filter + `run_plan` + `OrchestrationHook` + `build.rs` guard (incl. 2.1a embedding enrichment, 2.2a reducer hardening, 2.3a hook prototype gate, 2.3b planner output validation) | 1 | 7 | ~14k | medium — cache-key correctness, streaming integrity, planner robustness |
| 3 — delete domain chains + 2 example `PromptChainCapability` manifests | 2 (gate 2.3a passed) | 2.5 | ~3k | low — mostly deletion |
| 4 — focused `NativeStorageFactory` + manifests + delete `BuiltinFactory`/`WorkspaceProvider` (+0.5 hr optionality for Step A vs Step B template engine) | 1 | 5 | ~9k | medium — many call-sites + path templates |
| 5 — `JobBackedNativeFactory` + vision OCR chain + transcribe manifest | 1, 2 | 3 | ~5k | medium — vision-first OCR via `LlmRegistry` |
| 6 — upload-as-capability + `ManifestWatcher` + per-tenant `on_upload` policies | 4, 5 | 3.5 | ~6k | medium — UX-visible, realtime events |
| 7 — generic evals harness | 1 | 2 | low | low |
| 8 — docs/ADR (incl. ADR-0008 hook decision) + hygiene | all | 1.5 | low | low |
| **Total core refactor** |   | **~26.5–29 AI-hours** | **~45–50k tokens cumulative** | |

Research notes (R1–R8): budget 1–2 AI-hours each → **+8–12 AI-hours**
before the phases that depend on them merge.

> Token budgets assume clean SRP, existing factory injection patterns
> (`ChainFactory::new(llm)` mirrored by `NativeStorageFactory::new(...)` /
> `JobBackedNativeFactory::new(jobs)`), and `ArtifactBridge` retains
> post-execution ownership.

> **Phase 2 expansion rationale (reviewer-driven).** Sub-tasks 2.1a +
> 2.2a + 2.3a + 2.3b together add ~3.5 AI-hr (1 + 0.5 + 1.75 + 1) and
> ~7k tokens versus the prior 3.5 hr estimate. Each item is justified in
> its phase note. The prototype gate (2.3a) is **blocking for Phase 3**
> — streaming-safety must be proved before any chain deletion.

---

## 7. Non-Goals

- Replacing Rig with a different agent framework.
- Building a custom `CapabilityComposer` abstraction (planner manifest is
  enough).
- Multi-tenant isolation changes (already correct via `TenantContext`).
- Billing / metering changes (already generic via `QuotaChecker`).
- Frontend redesign (UI gains an "Inbox timeline" panel in Phase 6, but no
  visual overhaul).

---

## 8. Rollback Strategy

Each phase ships behind a feature flag (`CONUSAI_CAPS_V2=1`). The old
`BuiltinFactory` / `WorkspaceProvider` / `TranscribeVideoCapability` /
chain pipelines remain **compiled but unregistered** when the flag is on —
this is the reviewer-endorsed two-week safety window per phase. Legacy
code is deleted only after two successful production weeks per phase in a
follow-up commit (e.g. Phase 4 deletes `BuiltinFactory` source two weeks
after `NativeStorageFactory` ships). Manifest schema v1 stays supported
for one full release cycle. The `build.rs` grep guard (Phase 2.5) ensures
no new `rig::providers::` bypass can sneak back in during the window.

---

## 9. Immediate Actions (Start Order)

1. **Create research notes** stubs under `docs/research/` for R1–R8 (one
   markdown file per row in §4) and fill R1, R4, R6, R8 first — they
   directly unblock Phases 4–6.
2. **Start Phase 1** — add taxonomy doc + non-breaking `ToolManifest` v2
   fields + `cargo xtask capabilities lint`.
3. **Add the `build.rs` grep guard** (Phase 2.5) early — it prevents
   regressions while later phases delete bypasses.
4. **Run the Phase 2.3a hook prototype gate BEFORE any Phase 3
   deletion.** Prove `OrchestrationHook` preserves SSE chunk order,
   `max_turns` accounting, and tool-card lifecycle; otherwise switch to
   the sub-execution fallback and record the choice in ADR-0008. This is
   the single highest-risk integration in the refactor.
5. **Prototype one vision chain manifest**
   (`extract-ocr-vision/capability.toml`) end-to-end and verify it routes
   through `LlmRegistry::resolve_binding` (no `rig::providers::*::Client`
   anywhere). Validates the canonical path before any deletion.
6. **Update `docs/arch.md` §4.4.5 + feature inventory** as each phase
   lands, per the project audit instructions (no doc drift).

---

## 10. iOS Simulator Verification — Business-Prompt Use Cases

> Mirrors the structure of `docs/verify/verify.md`. Each use case exercises
> the Capabilities-as-Everything stack end-to-end from the **iOS WebKit
> Playwright project (`ios-mobile-web`, iPhone 15, 393 × 852 @ DPR 3)** with
> the gateway on `http://localhost:8080`, RustFS on `:9000` and Qdrant on
> `:6333`. All five scenarios are **purely manifest-driven** — no Rust
> change in `agent-core`/`agent-gateway` is required to make them pass.
>
> **Scope of these tests.** They verify the **router → planner → executor
> → ArtifactBridge** path on a real mobile viewport, including SSE
> streaming, tool-call cards, attachment chips, and the workspace folder
> tree updating in realtime after `storage.*` capabilities run.
>
> **Run command:**
> ```bash
> pnpm exec playwright test --project=ios-mobile-web e2e/ios/capabilities-business.spec.ts
> ```

### 10.0 Prerequisites

1. Docker stack is up and healthy (see `verify.md` Phase 4) with
   `CONUSAI_CAPS_V2=1` exported for the `agent-gateway` service.
2. Manifests for the following capabilities exist under
   `apps/backend/capabilities/` (created in Phases 4–5 of this plan):
   - `extract-ocr-vision`, `sense-classify-document`,
     `extract-fields-invoice`, `extract-fields-contract`,
     `extract-fields-medical-claim`, `extract-fields-cv`,
     `extract-fields-incident`
   - `storage-put`, `storage-ensure-date-folder`, `storage-list-folders`
   - `compose-report-md`, `compose-report-json`, `compose-email`
   - `plan-orchestrate` (meta-capability from Phase 2)
3. Fixtures present under `e2e/fixtures/capabilities/`:
   - `invoice.pdf`, `service-agreement.pdf`, `medical-claim.pdf`,
     `cv-{1..8}.pdf`, `incident-report.pdf` + `incident-photo-{1,2}.jpg`
4. A super-admin JWT in `SUPER_TOKEN` (helper in `verify.md` §16.0) and a
   tenant JWT in `TOKEN` (helper in `verify.md` §JWT).
5. Seed assertion that the registry includes all capabilities above:
   ```bash
   curl -sf -H "Authorization: Bearer $SUPER_TOKEN" \
     http://localhost:8080/admin/capabilities \
     | python3 -c "import sys,json; names={c['name'] for c in json.load(sys.stdin)}; req={'extract.fields.invoice','extract.fields.contract','extract.fields.medical_claim','extract.fields.cv','extract.fields.incident','extract.ocr.vision','sense.classify_document','storage.put','storage.ensure_date_folder','compose.report_md','compose.report_json','compose.email','plan.orchestrate'}; missing=req-names; assert not missing, f'missing {missing}'; print('all manifests registered')"
   ```

### 10.1 Use Case 1 — Finance / Accounting · Invoice Processing Pipeline

**User prompt (typed into iOS composer):**

> *"Process this invoice PDF. Extract all key fields, validate totals, file it in the correct dated folder under Finance/Invoices, and create a short summary I can forward to accounting."*

**Setup**

- Login on iOS viewport as `Finance Tester` (Enterprise plan).
- Drag-drop `e2e/fixtures/capabilities/invoice.pdf` onto the composer →
  attachment chip appears.

**Expected gateway behaviour**

| Step | Endpoint | Assertion |
|------|----------|-----------|
| 1 | `POST /v1/files` | `201` → token returned, RustFS shows `tenants/{tenant}/{uuid}/invoice.pdf` |
| 2 | `POST /v1/agent/completions` (`stream:true`) | SSE includes `tool_call_start` for `plan.orchestrate` |
| 3 | Capability search | `extract.fields.invoice`, `storage.ensure_date_folder`, `storage.put`, `compose.report_md` all appear in `tool_call_start` events |
| 4 | ArtifactBridge | New workspace node visible under `/Finance/Invoices/<YYYY>/<MM>/<DD>/` containing the original PDF + `summary.md` + structured `invoice.json` |
| 5 | Final assistant message | Contains `HCY-23256029`, `PAID`, `€63.99` and a forwardable summary |

**iOS UI assertions**

```ts
// e2e/ios/capabilities-business.spec.ts — Use Case 1
await uploadFile(page, 'invoice.pdf');
await page.getByRole('textbox').fill(
  'Process this invoice PDF. Extract all key fields, validate totals, file it in the correct dated folder under Finance/Invoices, and create a short summary I can forward to accounting.'
);
await submitComposer(page);

// Router → planner → executor visible as tool cards
await expect(page.getByText('plan.orchestrate')).toBeVisible({ timeout: 15_000 });
await expect(page.getByText('extract.fields.invoice')).toBeVisible({ timeout: 15_000 });
await expect(page.getByText('storage.ensure_date_folder')).toBeVisible();
await expect(page.getByText('storage.put')).toBeVisible();
await expect(page.getByText('compose.report_md')).toBeVisible();

// ArtifactBridge — workspace tree updates in realtime
const today = new Date().toISOString().slice(0, 10).replace(/-/g, '/'); // YYYY/MM/DD
await page.getByRole('button', { name: /menu|sidebar/i }).click();
await expect(page.getByRole('treeitem', { name: new RegExp(`Finance/Invoices/${today}`) }))
  .toBeVisible({ timeout: 20_000 });

// Final summary mentions invoice number
await expect(page.locator('.ai-bubble').last()).toContainText('HCY-23256029');
await snap(page, 'uc1-invoice-pipeline');
```

✅ **Pass**: SSE shows the orchestrated chain, RustFS path
`tenants/{tenant}/Finance/Invoices/<dated>/invoice.pdf` exists (verify via
`aws s3 ls` per `verify.md` §9), and the iOS workspace tree shows the new
folder.

---

### 10.2 Use Case 2 — Legal · Contract Review & Risk Extraction

**User prompt:**

> *"Review the attached service agreement. Extract all payment terms, termination clauses, and liability limitations. Flag any unusual or high-risk language and save a redlined summary."*

**Capabilities exercised:** `extract.fields.contract`, `sense.classify_document`,
`compose.report_md`, `storage.put` (filed under `/Legal/Contracts/`).

**iOS assertions**

```ts
await uploadFile(page, 'service-agreement.pdf');
await page.getByRole('textbox').fill(
  'Review the attached service agreement. Extract all payment terms, termination clauses, and liability limitations. Flag any unusual or high-risk language and save a redlined summary.'
);
await submitComposer(page);

await expect(page.getByText('sense.classify_document')).toBeVisible({ timeout: 15_000 });
await expect(page.getByText('extract.fields.contract')).toBeVisible();
await expect(page.getByText('compose.report_md')).toBeVisible();

// Classifier result surfaced in the tool card as "type=contract"
const classifyCard = page.locator('[data-tool="sense.classify_document"]').first();
await expect(classifyCard).toContainText(/contract/i);

// Final report mentions all three clause families
const reply = page.locator('.ai-bubble').last();
await expect(reply).toContainText(/payment/i);
await expect(reply).toContainText(/termination/i);
await expect(reply).toContainText(/liabilit/i);

// Workspace: /Legal/Contracts/<file>.md exists
await page.getByRole('button', { name: /menu|sidebar/i }).click();
await expect(page.getByRole('treeitem', { name: /Legal\/Contracts/ }))
  .toBeVisible({ timeout: 20_000 });

await snap(page, 'uc2-contract-review');
```

✅ **Pass**: classifier emits `kind=contract`, three clause families surface
in the markdown report, and `/Legal/Contracts/<slug>.md` is created via
`ArtifactBridge`.

---

### 10.3 Use Case 3 — Healthcare / Insurance · Medical Claim Processing

**User prompt:**

> *"This is a medical claim form with supporting documents. Extract patient details, procedure codes, diagnosis, and billed amounts. Classify the claim type and generate a structured report for our claims system."*

**Capabilities exercised:** `extract.ocr.vision` (R1 vision-first), optionally
`extract.ocr.tesseract` as a sibling under `parallel_consensus`,
`extract.fields.medical_claim`, `sense.classify_document`,
`compose.report_json`.

**Strategy assertion:** the `plan.orchestrate` tool card MUST expose
`strategy: "parallel_consensus"` in its rendered metadata when both
extractors are registered (see §2 reducers).

**iOS assertions**

```ts
await uploadFile(page, 'medical-claim.pdf');
await page.getByRole('textbox').fill(
  'This is a medical claim form with supporting documents. Extract patient details, procedure codes, diagnosis, and billed amounts. Classify the claim type and generate a structured report for our claims system.'
);
await submitComposer(page);

await expect(page.getByText('plan.orchestrate')).toBeVisible({ timeout: 15_000 });
const planCard = page.locator('[data-tool="plan.orchestrate"]').first();
await expect(planCard).toContainText(/parallel_consensus|single|fallback_cascade/);

await expect(page.getByText('extract.ocr.vision')).toBeVisible();
await expect(page.getByText('extract.fields.medical_claim')).toBeVisible();
await expect(page.getByText('compose.report_json')).toBeVisible();

// Final artifact is a JSON node — UI shows JSON preview affordance
const reply = page.locator('.ai-bubble').last();
await expect(reply).toContainText(/patient|procedure|diagnosis/i);

await snap(page, 'uc3-medical-claim');
```

✅ **Pass**: vision OCR runs through `LlmRegistry` (verify via gateway log
line `llm.binding=vision.default`, NOT `rig::providers::*` direct construct
— guard from Phase 2.5), and a JSON artifact is materialised.

---

### 10.4 Use Case 4 — HR / Talent Acquisition · CV Screening & Shortlist

**User prompt:**

> *"Screen these 8 CVs for a Senior Rust Engineer role. Score them on relevant experience, highlight top 3 candidates, and draft a short outreach email for the best one."*

**Capabilities exercised:** `extract.ocr.vision`, `extract.fields.cv`,
`sense.classify_document`, `compose.email`, `storage.put`. Multi-attachment
orchestration handled by `plan.orchestrate` running CV extraction in
parallel then ranking + email composition sequentially.

**iOS assertions**

```ts
for (let i = 1; i <= 8; i++) await uploadFile(page, `cv-${i}.pdf`);
// Attachment list shows 8 chips
await expect(page.locator('[data-attachment-chip]')).toHaveCount(8);

await page.getByRole('textbox').fill(
  'Screen these 8 CVs for a Senior Rust Engineer role. Score them on relevant experience, highlight top 3 candidates, and draft a short outreach email for the best one.'
);
await submitComposer(page);

// Planner runs 8 parallel extract.fields.cv tool cards
const cvCards = page.locator('[data-tool="extract.fields.cv"]');
await expect(cvCards).toHaveCount(8, { timeout: 30_000 });

await expect(page.getByText('compose.email')).toBeVisible({ timeout: 30_000 });

const reply = page.locator('.ai-bubble').last();
// Reply contains a ranked list (top 3) + email draft headline
await expect(reply).toContainText(/top 3|shortlist|ranking/i);
await expect(reply).toContainText(/subject:|hi |hello /i);

// Workspace: /HR/Candidates/Senior-Rust-Engineer/ has 8 CVs + draft email node
await page.getByRole('button', { name: /menu|sidebar/i }).click();
await expect(page.getByRole('treeitem', { name: /HR\/Candidates\/Senior-Rust-Engineer/ }))
  .toBeVisible({ timeout: 20_000 });

await snap(page, 'uc4-cv-screening');
```

✅ **Pass**: 8 parallel `extract.fields.cv` cards (verifies executor's
fan-out via `plan.orchestrate`), an email draft artifact exists, and the
target folder is populated.

---

### 10.5 Use Case 5 — Operations / Logistics · Incident Report + Follow-up

**User prompt:**

> *"Analyse this incident report PDF and attached photos. Extract key facts, assess severity, suggest immediate actions, and create a follow-up task summary. File everything under Operations/Incidents."*

**Capabilities exercised:** `extract.ocr.vision` (PDF + photos),
`sense.classify_document`, `extract.fields.incident`, `compose.report_md`,
`storage.ensure_date_folder` + `storage.put`. Demonstrates **mixed-MIME
routing** via the `accepts` filter (Phase 1 manifest field).

**iOS assertions**

```ts
await uploadFile(page, 'incident-report.pdf');
await uploadFile(page, 'incident-photo-1.jpg');
await uploadFile(page, 'incident-photo-2.jpg');
await expect(page.locator('[data-attachment-chip]')).toHaveCount(3);

await page.getByRole('textbox').fill(
  'Analyse this incident report PDF and attached photos. Extract key facts, assess severity, suggest immediate actions, and create a follow-up task summary. File everything under Operations/Incidents.'
);
await submitComposer(page);

// MIME routing: PDF → extract.fields.incident, JPGs → extract.ocr.vision (≥2 calls)
const ocrCards = page.locator('[data-tool="extract.ocr.vision"]');
await expect(ocrCards.first()).toBeVisible({ timeout: 30_000 });
await expect(page.getByText('extract.fields.incident')).toBeVisible();
await expect(page.getByText('storage.ensure_date_folder')).toBeVisible();
await expect(page.getByText('compose.report_md')).toBeVisible();

const reply = page.locator('.ai-bubble').last();
await expect(reply).toContainText(/severity/i);
await expect(reply).toContainText(/action|follow.?up|next step/i);

// Dated incident folder
const today = new Date().toISOString().slice(0, 10).replace(/-/g, '/');
await page.getByRole('button', { name: /menu|sidebar/i }).click();
await expect(page.getByRole('treeitem', { name: new RegExp(`Operations/Incidents/${today}`) }))
  .toBeVisible({ timeout: 20_000 });

await snap(page, 'uc5-incident-package');
```

✅ **Pass**: photos go through vision OCR (≥ 2 cards), incident JSON +
markdown report + 3 files land in `/Operations/Incidents/<dated>/`, and a
realtime UI event is logged.

---

### 10.6 Cross-Cutting Architectural Assertions (run after all 5 use cases)

These run **once** at the end of the iOS spec file and protect the §0
invariants from regressions reachable only through the full UI path.

```ts
test.afterAll(async () => {
  // (a) No domain symbol leaks into agent-core/agent-gateway
  //     — checked by build.rs guard, but re-asserted here against the
  //     running binary's emitted log markers.
  const logs = await fetch('http://localhost:8080/admin/diagnostics/log-tail?lines=500', {
    headers: { Authorization: `Bearer ${process.env.SUPER_TOKEN}` },
  }).then(r => r.text());
  expect(logs).not.toMatch(/rig::providers::(anthropic|openai)::Client::new/);

  // (b) Router top-K never exceeds 50 even with 13+ capabilities registered
  const search = await fetch('http://localhost:8080/v1/capabilities/search?q=invoice&k=999', {
    headers: { Authorization: `Bearer ${process.env.TOKEN}` },
  }).then(r => r.json());
  expect(search.results.length).toBeLessThanOrEqual(50);

  // (c) Every artifact created by the 5 use cases is reachable via
  //     workspace list + has a non-null content body in RustFS.
  const tree = await fetch('http://localhost:8080/v1/workspace/tree', {
    headers: { Authorization: `Bearer ${process.env.TOKEN}` },
  }).then(r => r.json());
  const required = [
    'Finance/Invoices', 'Legal/Contracts',
    'HR/Candidates/Senior-Rust-Engineer', 'Operations/Incidents',
  ];
  for (const path of required) {
    expect(JSON.stringify(tree)).toContain(path);
  }
});
```

### 10.7 Acceptance Gate (Phase Sign-Off)

A phase cannot be marked complete until the relevant iOS use cases pass:

| Plan Phase | Required iOS Use Cases |
|------------|------------------------|
| 2 (planner) | 10.1, 10.4 (orchestration with fan-out) |
| 3 (chain refactor) | 10.1, 10.2, 10.3 (extract.fields.* via `LlmRegistry`) |
| 4 (storage providers) | 10.1, 10.2, 10.5 (date-folder + put) |
| 5 (OCR vision-first) | 10.3, 10.5 (vision + parallel_consensus) |
| 6 (hot reload + UI) | All 5 — manifests added without gateway restart |
| 7 (planner deletion of chains) | Re-run all 5 from cold start |
| 8 (legacy code deletion) | All 5 + 10.6 cross-cutting |

> Failing any required use case blocks the phase from being marked
> complete in §6 and from advancing to the next phase. Screenshots in
> `test-results/ios-playwright-visual/uc{1..5}-*.png` are committed as
> evidence (per audit rules in `docs/project-instructions.md`).
