# Capability Authoring Guide — Gap Closure Plan v0.3

> Source: cross-check of [capability-authoring-guide.md](capability-authoring-guide.md)
> against the live code in `apps/backend/...` (2026-05-09).
> Replaces v0.2 after a second review pass; folds in the v0.3 refinements
> (proptest, CI guard, startup-warning form, full `ToolKind` documentation)
> and explicitly **defers** the proposed `ToolRegistry` → `CapabilityRegistry`
> rename as scope creep — see §4.
>
> **v0.3 changelog vs v0.2:**
> - PR-1: method named `enabled_for_tenant` (was `all_enabled_for`); add
>   proptest coverage for the scope filter; rename rejected (see §4).
> - PR-2: startup check uses `var_os(…).map_or(true, |v| v.is_empty())` and
>   structured `tracing` field for ops alerting.
> - PR-3: add a CI guard that fails the build if the stale root
>   `capabilities/` or `crates/` directories reappear; explicitly require
>   the guide to list **all** `ToolKind` variants (not just `dynamic_prompt`).

---

## 0. Audit corrections (v0.1 → v0.2)

The first pass flagged TOML-vs-YAML as a real gap. It is **not** — the active
backend at [apps/backend/capabilities](apps/backend/capabilities/) ships
`capability.toml` files exactly as the guide describes:

```
apps/backend/capabilities/
├── contract-processing/capability.toml
├── invoice-processing/capability.toml
├── google-workspace/capability.toml
├── file-storage/capability.toml
├── ocr-service/capability.toml
├── runtime-echo/capability.toml
└── template-wasm/capability.{toml,wasm}
```

The loader at [tools/registry.rs](apps/backend/crates/agent-core/src/tools/registry.rs#L131)
hard-codes `capability.toml`. The root [capabilities/](capabilities/) directory
with `.yaml` files is stale and is **not** consumed by the active backend
(only the legacy root `crates/agent-gateway` referenced it, and the workspace
no longer builds it — only [apps/backend/crates/agent-gateway/Cargo.toml](apps/backend/crates/agent-gateway/Cargo.toml)
exists).

Result: the guide is correct; the **repo** has a stale tree to remove.

---

## 1. Confirmed gaps (only the real ones)

| # | Gap | Severity | Surface |
|---|---|---|---|
| G1 | Reference service uses `CONUSAI_PLATFORM_URL` / `CONUSAI_PLATFORM_TOKEN` / `CONUSAI_SERVICE_URL`; guide documents `GATEWAY_URL` / `PLATFORM_ADMIN_TOKEN` / `SERVICE_URL` | Medium — anyone copy-pasting the guide will get unregistered services | Docs + service env-var contract |
| G2 | `tenant_scope` is **stored** on every row and **propagated** into `ToolManifest`, but no routing/listing path filters by it. Tools with non-empty `tenant_scope` are exposed to every tenant | High — silent tenant leak of scoped tools | `routes/capabilities.rs`, `routes/search.rs`, agent tool selection |
| G3 | Stale root `capabilities/*.yaml` and root `crates/agent-gateway` tree | Low — confusion / lint noise | Repo cleanup |
| G4 | Guide does not mention `dynamic_prompt` `ToolKind` (it exists in [manifest.rs](apps/backend/crates/agent-core/src/tools/manifest.rs#L62-L65)) | Low — undocumented but functional surface | Docs |
| G5 | Guide claims Python example uses `GATEWAY_URL` / `PLATFORM_ADMIN_TOKEN`; real `current-time` service uses `CONUSAI_*` names → guide and reference example diverge | Medium (subset of G1) | Docs |
| G6 | No automated test that a registered `remote_mcp` capability is reachable end-to-end (register → LISTEN/NOTIFY reload → `tools/list` from gateway → invoke) | Medium — regression risk on the hot-reload path | `apps/backend/crates/agent-gateway/tests/` |
| G7 | `PLATFORM_ADMIN_TOKEN` is read at request time inside the handler ([admin_capabilities.rs](apps/backend/crates/agent-gateway/src/routes/admin_capabilities.rs#L560-L575)); env value can drift from the rest of the gateway config; no startup warning when unset in non-dev mode | Low — config-hygiene | `state.rs`, handler |

Items G1, G2, G6 are merge-blocking for "production-ready" claims in the
guide. G3–G5, G7 are quality / cleanup.

---

## 2. Implementation plan

Three small PRs, each independently mergeable. Total effort: **~6–8 AI-hours**.

### PR-1 — Tenant scope enforcement (G2)  ·  ~3 AI-h

**Goal:** every code path that lists or selects capabilities filters out rows
whose `tenant_scope` is non-empty *and* does not contain the resolved tenant.

> **v0.3 decision — NO renames.** The v0.3 review proposed renaming
> `ToolRegistry` → `CapabilityRegistry` and moving `tools/registry.rs` →
> `capabilities/registry.rs`. Rejected: the rename touches **47+ call sites**
> across `tools/admin.rs`, `tools/discovery.rs`, `tools/executor.rs`,
> `tools/semantic_router.rs`, `tools/providers/capability_spec.rs`,
> `agent/runtime.rs`, and `agent-gateway/src/state.rs`, and creates an
> inconsistency where one sibling lives in `capabilities/` while the rest of
> the module (`tools/admin.rs`, `tools/executor.rs`, …) stays under `tools/`.
> Renaming the whole `tools/` module is real scope creep that does not close
> a single gap from G1–G7. Tracked under §4 as a separate, deferred refactor.

Files & exact changes:

1. [tools/registry.rs](apps/backend/crates/agent-core/src/tools/registry.rs) —
   add helper (canonical method name `enabled_for_tenant`, mirrors the
   v0.4 vocabulary used elsewhere on `CapabilityCard`):
   ```rust
   impl ToolRegistry {
       /// Iterator over enabled cards visible to `tenant_id`.
       /// Empty `tenant_scope` = global; otherwise membership is required.
       pub fn enabled_for_tenant(&self, tenant_id: &str)
           -> impl Iterator<Item = &CapabilityCard>
       {
           self.all_enabled().filter(move |c| {
               let scope = &c.manifest().tenant_scope;
               scope.is_empty() || scope.iter().any(|t| t == tenant_id)
           })
       }
   }
   ```
2. [routes/capabilities.rs](apps/backend/crates/agent-gateway/src/routes/capabilities.rs) —
   replace `all_enabled()` with `enabled_for_tenant(&tenant.0.tenant_id)`.
3. [routes/search.rs](apps/backend/crates/agent-gateway/src/routes/search.rs#L42-L60) —
   same swap before passing `cards` into `vector_search` / `local_search`.
4. Agent tool-selection path — grep for callers of `registry.all_enabled()`
   inside `agent` / `chat` handlers and the
   [SemanticCapabilityRouter](apps/backend/crates/agent-core/src/tools/semantic_router.rs#L88-L100)
   constructor; thread the tenant id through and filter at the same point.
5. SQL safety net: extend the search SQL in
   [search.rs](apps/backend/crates/agent-gateway/src/routes/search.rs)
   `top_n_capabilities` to add
   `WHERE cardinality(tenant_scope) = 0 OR $tenant = ANY(tenant_scope)` so
   ANN cannot leak even if the in-memory filter is bypassed. Confirm the
   existing GIN index `capability_specs_scope_idx` is used (`EXPLAIN`).
6. Tests:
   - `tests/tenant_scope.rs` — register two `remote_mcp` tools, one with
     `tenant_scope=["acme"]`, assert tenant `other` cannot list/search/invoke
     it but `acme` can.
   - `tests/tenant_scope_property.rs` — **proptest** over
     `(scope: Vec<String>, tenant_id: String)`: invariant
     `scope.is_empty() || scope.contains(&tenant_id)` ⇔ visible. Locks the
     filter against future regressions cheaply.

Acceptance:
- Both tests green; proptest runs ≥ 256 cases by default.
- `cargo clippy -p agent-gateway -- -D warnings` clean.
- Manual: `current-time` registered with `tenant_scope=["acme"]` is invisible
  on `GET /v1/capabilities` for `X-Tenant-ID: other`.
- No behavioural change for capabilities with empty `tenant_scope`.

### PR-2 — Env-var contract alignment (G1, G5, G7)  ·  ~2 AI-h

**Goal:** one canonical set of names; reference service, guide, and handler
match exactly.

Decision: **adopt the guide's names** (`GATEWAY_URL`, `PLATFORM_ADMIN_TOKEN`,
`SERVICE_URL`). They are shorter, vendor-neutral, and already documented.

Files & changes:

1. [services/current-time/main.py](services/current-time/main.py#L100-L150) —
   accept the new names, keep `CONUSAI_PLATFORM_URL` / `CONUSAI_PLATFORM_TOKEN`
   / `CONUSAI_SERVICE_URL` as fallbacks for one release with a deprecation
   `print(...)` on use.
2. [docker-compose.yml](docker-compose.yml) `services.current-time.environment`
   — set the new names; keep old as commented-out for migration.
3. [admin_capabilities.rs](apps/backend/crates/agent-gateway/src/routes/admin_capabilities.rs#L560)
   — keep reading `PLATFORM_ADMIN_TOKEN`; add a startup check in
   [state.rs](apps/backend/crates/agent-gateway/src/state.rs) using
   `var_os` (avoids a needless `String` alloc and treats invalid UTF-8
   identically to "unset") with a structured `tracing` field for ops
   dashboards:
   ```rust
   if std::env::var_os("PLATFORM_ADMIN_TOKEN")
       .map_or(true, |v| v.is_empty())
       && !cfg!(debug_assertions)
   {
       tracing::warn!(
           config = "missing",
           env = "PLATFORM_ADMIN_TOKEN",
           "/admin/capabilities/register is OPEN in a non-debug build"
       );
   }
   ```
4. [capability-authoring-guide.md](capability-authoring-guide.md) — no edits
   needed (it's already authoritative); add a short "Env-var migration" note
   under §Environment variables listing the deprecated `CONUSAI_*` aliases.

Acceptance:
- `docker compose up` end-to-end registers `current-time` with the new names.
- Setting only the deprecated names still works and emits one deprecation log
  line.
- Production startup with no `PLATFORM_ADMIN_TOKEN` produces a warning.

### PR-3 — End-to-end remote_mcp test + cleanup + full ToolKind docs (G3, G4, G6)  ·  ~2 AI-h

**Goal:** lock in the hot-reload contract; remove stale code; document
every `ToolKind` variant.

Files & changes:

1. New [apps/backend/crates/agent-gateway/tests/remote_mcp_e2e.rs](apps/backend/crates/agent-gateway/tests):
   spin up a `wiremock` server that answers `tools/list` and `tools/call`,
   register it via `POST /admin/capabilities/register`, wait for the
   `RealtimeService` reload using `tokio::time::timeout(Duration::from_secs(2), …)`
   over a poll of `GET /v1/capabilities`, then invoke via the agent tool path
   and assert the wiremock saw the call. Pinned regression for the
   LISTEN/NOTIFY → `reload_one` → `invalidate_all` chain.
2. Delete the stale root [capabilities/](capabilities/) `*.yaml` directory and
   the stale root [crates/](crates/) tree if `cargo metadata` confirms they
   are not workspace members. (Verify first with
   `cargo metadata --format-version 1 | jq '.workspace_members'`; do **not**
   delete unless absent.)
3. **CI guard** (new) — add a `lint-no-stale-trees` job to
   `.github/workflows/ci.yml` that fails if `git ls-files | grep -E '^(capabilities/|crates/)'`
   ever returns a match again. Cheap, prevents regression from a stray
   `git mv`.
4. [capability-authoring-guide.md](capability-authoring-guide.md) §Option A:
   replace the lone TOML example with a one-line bullet per **every** real
   `ToolKind` variant from
   [manifest.rs](apps/backend/crates/agent-core/src/tools/manifest.rs#L55-L67):
   `chain`, `wasm`, `native`, `mcp`, `docker`, `dynamic_prompt`, `remote_mcp`.
   Each bullet links to the variant and gives one sentence on when to choose it.

Acceptance:
- `cargo test -p agent-gateway --test remote_mcp_e2e` green in < 5s.
- `git ls-files capabilities/ crates/ | wc -l` returns 0 after cleanup.
- CI `lint-no-stale-trees` job is green and would fail if either tree
  reappeared (verified by a temporary `touch capabilities/x` run on a branch).
- Guide references **all** real `ToolKind` variants.

---

## 3. Sequencing

```
PR-1 (tenant scope)  ──► PR-3 (e2e + cleanup)
PR-2 (env contract)  ──► PR-3 (uses canonical env in the test)
                              │
                              ▼
           [genericization epic, §5]  Phase 1.1 starts after PR-3 merges.
```

PR-1 and PR-2 are independent and can run in parallel. PR-3 depends on both.
The genericization epic in §5 starts **only after** all three merge — it
inherits tenant isolation (PR-1), the canonical env contract (PR-2), and the
hot-reload regression net (PR-3) as preconditions, which materially de-risks
its 25–32 AI-h scope.

### Explicit non-impact on the invoice flow

PR-1/2/3 deliberately leave the existing invoice / contract pipeline
**unchanged**. After they merge, the upload→extract path is still:

- `POST /ui/extract-invoice` ([ui/handlers/invoice.rs](apps/backend/crates/agent-gateway/src/ui/handlers/invoice.rs))
- `InvoicePipeline` ([chains/invoice.rs](apps/backend/crates/agent-core/src/chains/invoice.rs))
- `match tool_name { "extract_invoice" => … }` in [tools/providers/chain.rs:45-180](apps/backend/crates/agent-core/src/tools/providers/chain.rs#L45-L180)
- Frontend `isInvoice()` heuristics + `invoiceCard()`

This is intentional — those four sites are deleted by the genericization epic
(§5), not by this gap plan. Mixing them in would re-introduce exactly the
scope creep we rejected in PR-1.

## 4. Out of scope (deliberately deferred)

- **`ToolRegistry` → `CapabilityRegistry` rename + `tools/` → `capabilities/`
  module move** (proposed in v0.3 review). Rejected for this epic: 47+ call
  sites across `tools/admin.rs`, `tools/discovery.rs`, `tools/executor.rs`,
  `tools/semantic_router.rs`, `tools/providers/capability_spec.rs`,
  `agent/runtime.rs`, and `agent-gateway/src/state.rs`. A partial rename
  (registry only) creates a worse split than the status quo. If the team
  wants v0.4 vocabulary alignment, do the **whole** module rename in a
  dedicated PR with no behavioural changes — trivial to review, easy to
  revert, doesn't block any production-blocking gap.
- Migrating self-registration from a root JSON POST to a signed JWT (covered
  by [docs/plan.md](plan.md) Phase D).
- Postgres RLS for `capability_specs.tenant_scope` (PR-1's SQL `WHERE` is
  sufficient until multi-process gateway scale demands it).
- Replacing `fastembed` local mode with a hosted embedder — orthogonal.
- Removing `MINIO_*` / `S3_*` env-var fallbacks in
  [state.rs](apps/backend/crates/agent-gateway/src/state.rs) (separate
  cleanup epic).

## 5. Adjacent epic: generic prompt-capability architecture (cross-link, NOT in scope)

A separate review proposes finishing the v0.4 "domain logic in TOML, not Rust"
refactor (Phase 1-4: extend manifest with `intent_hints` + `result_view`, port
invoice/contract to `DynamicPrompt`, delete the bespoke Rust pipelines, add a
generic Svelte `ToolResultCard`). Codebase reality check (2026-05-10):

| Claim in suggestion | Codebase truth |
|---|---|
| "New `PromptCapabilityFactory`" | Already shipped as [`DynamicPromptFactory`](apps/backend/crates/agent-core/src/tools/providers/dynamic_prompt.rs) + [`DynamicPromptCapability`](apps/backend/crates/agent-core/src/chains/dynamic_prompt.rs) (v0.3.2). Use it as-is; do **not** add a parallel factory. |
| "Extend `LlmChainConfig` with `output_schema`, vision, model" | All three fields already exist on [`LlmChainConfig`](apps/backend/crates/agent-core/src/tools/manifest.rs#L7-L25). |
| "Auto-generate Rig tool schema from `input_schema`" | `ToolDef.input_schema` is already JSON Schema; Rig conversion is a 1-call wrapper, not a new abstraction. |
| "Delete `chains/invoice.rs`, `chains/contract.rs`, the `match` in `tools/providers/chain.rs`, and `ui/handlers/invoice.rs`" | These **do** still exist ([chain.rs:45-180](apps/backend/crates/agent-core/src/tools/providers/chain.rs#L45-L180), [ui/handlers/invoice.rs](apps/backend/crates/agent-gateway/src/ui/handlers/invoice.rs)) — real work, **separate epic**. |
| "Add `intent_hints` + `result_view` manifest blocks" | Not present today. Worth adding, but it changes the manifest contract — belongs with the port, not with G1-G7. |
| "On-boot eval hook" | The `evals/` crate exists; no on-boot hook. Cheap (~2 AI-h) but orthogonal. |
| "Qdrant embedding of `intent_hints`" | Semantic router already embeds descriptions; adding `intent_hints` is a one-field append once the manifest is extended. |

**Decision:** keep this gap plan tight on G1-G7 (operational hardening, ~7 AI-h).
Track the genericization as a dedicated epic (~25-32 AI-h) under
[docs/plan.md](plan.md) or a new `docs/capability-genericization-plan.md`.
Two reasons:
1. Mixing themes hides the production-blocker fixes (G2 tenant leak) behind a
   30-hour refactor.
2. The genericization PRs have a different blast radius and need their own
   feature flag + parallel-run-then-cut-over period — incompatible with the
   "three small mergeable PRs" shape of this plan.

**Light-touch carry-overs adopted here:** the on-boot eval hook will be
listed as a follow-up acceptance for PR-3 *only if* the broader epic kicks
off in the same milestone. Otherwise it stays out.

---

## 6. Definition of done

1. PR-1, PR-2, PR-3 merged; CI green including the new
   `tenant_scope` (+ proptest), `remote_mcp_e2e`, and `lint-no-stale-trees`
   jobs.
2. Reference service ([services/current-time/main.py](services/current-time/main.py))
   uses the canonical env-var names; `docker compose up` self-registers cleanly
   and the deprecated `CONUSAI_*` names still work for one release with a
   one-time deprecation log line.
3. `GET /v1/capabilities`, `GET /v1/capabilities/search`, and the agent
   tool-selection path never return a capability whose non-empty
   `tenant_scope` excludes the caller's tenant. Verified by SQL `EXPLAIN`
   showing the GIN index hit on `capability_specs_scope_idx`.
4. The repo no longer contains stale root `capabilities/*.yaml` or root
   `crates/agent-gateway` artifacts; the CI guard prevents reintroduction.
5. [capability-authoring-guide.md](capability-authoring-guide.md) documents
   **all** `ToolKind` variants and the deprecated env-var aliases.
6. Production startup with no `PLATFORM_ADMIN_TOKEN` emits a structured
   `tracing::warn!` with `config="missing", env="PLATFORM_ADMIN_TOKEN"`
   suitable for ops alerting.
