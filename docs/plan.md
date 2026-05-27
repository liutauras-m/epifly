# Backend Refactor Plan — Agent Gateway & Workspaces (v3.3)

Driven by [docs/suggestion.md](docs/suggestion.md) + five rounds of reviewer feedback. Restructured into **5 phases** with sharper sequencing to keep blast radius small. Each step is independently executable by an AI coding agent.

**v3 deltas vs v2:** twin `VirtualPath` containment methods (`is_strict_child_of` vs `is_same_or_within`); private path constructors; precise non-tool-model rules; tokenizer strategy; `request_id` across retries; module-direction rule for Step 2.1→2.7; typed `AgentEvent` sink (no `Bytes`); Phase 3 honestly scoped to include agent indexing call sites; dual-write failure semantics; `tokio::join!` instead of `try_join!` for best-effort cleanup; Phase 4 renamed to emphasize provider boundary over Rig.

**v3.1 deltas vs v3:** corrected SSE expected order (`routing_meta` first); added explicit `ToolRoutingDecision` with `tool_required` rules; folder delete now captures `DeletePlan` before cascade; softened vector-store transaction language to be Qdrant-honest; added `ModelCatalog` alias compatibility; scoped `RedactPiiHook` to logs/audit by default; replaced `cargo deny` import-direction example with xtask/grep; fixed `virtual_path.rs` layout wording.

**v3.2 deltas vs v3.1:** Step 1.1 title no longer references the dead `is_within` name; Step 1.4 spells out resolve→route→gate execution order so text-only models skip tool-definition loading; `cleanup_after_delete` prefers `object_key` when present to avoid redoing the work in Phase 3; added explicit alias-resolution test requirement.

**v3.3 deltas vs v3.2:** Step 3.6 property tests now reference both final method names (no `is_within` ghost); Step 2.3 equivalence is "event-sequence equivalent" via typed `AgentEvent` assertions instead of brittle byte equality; execution checklist now carries an explicit scope-creep stop condition.

Primary files in scope:
- [apps/backend/crates/agent-gateway/src/routes/agent.rs](apps/backend/crates/agent-gateway/src/routes/agent.rs) (~1771 lines)
- [apps/backend/crates/agent-gateway/src/routes/workspaces.rs](apps/backend/crates/agent-gateway/src/routes/workspaces.rs) (~983 lines)
- [apps/backend/crates/agent-core](apps/backend/crates/agent-core)
- [apps/backend/crates/jobs](apps/backend/crates/jobs)

## Test gates

**Per step (fast):**
```
cargo fmt
cargo clippy -p <package-touched> --all-targets -- -D warnings
cargo test -p <package-touched>
```
For cross-crate changes use `--workspace`. For storage/migration steps, run testcontainer-backed integration tests (RustFS + Postgres + Qdrant).

**Per phase gate (slow, mandatory before next phase):**
```
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm test:e2e:web
```

**Never** run full `pnpm test:e2e:web` per commit — it's too slow and gets skipped. Use it only at phase boundaries.

---

## Phase 0 — Safety nets (no behavior change)

### Step 0.1 — Verify the "onboarding logic bug" claim; rename only
File: [workspaces.rs:266-283](apps/backend/crates/agent-gateway/src/routes/workspaces.rs#L266). `is_tenant_seeded()` returns `true` when seeded; the variable `still_unseeded` is **misnamed** but semantically holds `still_seeded`, and `if !still_unseeded` correctly provisions when unseeded. **Do not invert the condition.**
- Rename `seeded` → `is_seeded` and `still_unseeded` → `still_seeded`.
- Add unit test asserting `provision()` is called once when `is_tenant_seeded` returns `false`, zero times when `true`.
- Replace `unwrap_or("__dev__")` with `cfg(debug_assertions)`-gated dev fallback; production returns `HttpError::agent("tenant has no resolved user")`.

### Step 0.2 — Anthropic SSE upstream mock harness
Add `apps/backend/crates/agent-gateway/tests/anthropic_sse_mock.rs` using `wiremock`. Assert the gateway emits the expected delta sequence (`routing_meta` **first**, then interleaved `text` / `tool_start` / `tool_result`, ending with `done`) for a canned upstream stream. `routing_meta` must precede any model output — that is the current behavior and must be preserved. This is the safety net for every later `agent.rs` change.

### Step 0.3 — Presign path-confusion regression tests (RED)
Write tests **before** the fix in Step 1.1. These tests should fail against current code if unignored. Commit them ignored in Phase 0, then unignore in Step 1.1 after the fix lands. Cases to cover:
- User A presigns under accessible node B with `virtual_path = "/tenants/other/secret.txt"`.
- `virtual_path` = `/node/foobar` when node path is `/node/foo` (sibling-prefix attack — see Step 1.1).
- `virtual_path` containing `..`, `//`, percent-encoded traversal.
Land tests with `#[ignore = "fixed in Step 1.1"]` so CI stays green but the regressions are tracked. Do not commit code that demonstrates exploits against current behavior in CI output.

### Step 0.4 — Tenant isolation skeleton tests
Add `apps/backend/crates/agent-gateway/tests/tenant_isolation.rs` with `#[ignore]` cases for:
- `thread_id` from tenant B used under tenant A's JWT → expect 404.
- `workspace_node_id` from tenant B → expect 403.
- Forced-capability with unknown capability → expect rejection.
Skeleton now; flip from `#[ignore]` to enabled as each underlying fix lands.

---

## Phase 1 — Critical correctness & security

### Step 1.1 — Bind presigned path to node with typed `VirtualPath` containment methods
Files: [workspaces.rs](apps/backend/crates/agent-gateway/src/routes/workspaces.rs) `presign_upload` (~L753), `presign_download` (~L820+).

1. In `agent-core::workspace::virtual_path`, add **two** containment methods (distinct semantics matter):
   ```rust
   impl VirtualPath {
       /// True if `self` is the same path as `parent` OR a descendant of it.
       /// Use for content routes where the node IS the file.
       pub fn is_same_or_within(&self, parent: &VirtualPath) -> bool {
           let p = parent.components();
           let s = self.components();
           s.len() >= p.len() && s.iter().take(p.len()).eq(p.iter())
       }

       /// True only if `self` is a strict descendant of `parent` (not equal).
       /// Use for attachment/blob child uploads — prevents overwriting the
       /// parent's own content object.
       pub fn is_strict_child_of(&self, parent: &VirtualPath) -> bool {
           let p = parent.components();
           let s = self.components();
           s.len() > p.len() && s.iter().take(p.len()).eq(p.iter())
       }
   }
   ```
   **Do NOT use `str::starts_with`** — it would treat `/foo-bar` as inside `/foo`.
2. **`VirtualPath` constructors must be private or otherwise guarantee canonicalization.** Public construction goes through `parse` only. Add tests proving invalid paths cannot be constructed through any public API (no `pub` tuple struct, no `pub` `new(String)`, no `From<String>`).
3. `VirtualPath::parse` must canonicalize: reject `..`, empty segments, percent-encoded traversal, mixed separators.
4. For **content routes** where the node IS the file (Conversation / Document leaf), ignore client-supplied `virtual_path` and derive from `node.virtual_path`. Mark `PresignUploadBody.virtual_path` as `Option<String>` and document it as legacy-only.
5. For **attachment/blob routes** where the path must be a child of the node, enforce `requested.is_strict_child_of(&node.virtual_path)` (never `is_same_or_within` — that would allow overwriting the parent content object) else `HttpError::forbidden("virtual_path outside node")`.
6. Cap `body.size_bytes` against `plan.limits().max_upload_bytes` AND a hard server max (e.g. 500 MB).
7. Enforce `content_type` against an allowlist; reject unknown.
8. Enable the Step 0.3 tests (remove `#[ignore]`).

### Step 1.2 — Explicit tool-input JSON error (no silent `{}` fallback)
Files: [agent.rs:1324](apps/backend/crates/agent-gateway/src/routes/agent.rs#L1324), [agent.rs:1343](apps/backend/crates/agent-gateway/src/routes/agent.rs#L1343).
- Replace `serde_json::from_str(...).unwrap_or(json!({}))` with `match`. On `Err(e)`, push a `tool_result { is_error: true, content: "Invalid tool input JSON: {e}" }` and `continue` without invoking the tool.
- Apply to both blocking and streaming paths.

### Step 1.3 — Centralized HTTP client with retry semantics for LLM calls
- Add `state.http_upstream: reqwest::Client` built once: `timeout(90s)`, `connect_timeout(10s)`, `pool_idle_timeout(30s)`, `tcp_keepalive(60s)`.
- Replace `reqwest::Client::new()` at [agent.rs:609](apps/backend/crates/agent-gateway/src/routes/agent.rs#L609), [agent.rs:867](apps/backend/crates/agent-gateway/src/routes/agent.rs#L867) and everywhere else in agent-gateway.
- Retry rules (these supersede generic HTTP retry advice):
  - **Retry only model calls before any response bytes are received.**
  - **Never retry after a tool call has been emitted or executed.**
  - **Never retry a streaming response after the first upstream SSE event.**
  - Retry only `408`, `429`, `5xx`; honor `Retry-After`; max 2 attempts; exponential backoff with jitter.
- Metrics (OpenTelemetry):
  - `llm_upstream_retry_count{provider,model,status}`
  - `llm_upstream_timeout_count{provider,model}`
  - `llm_upstream_retry_exhausted_count{provider,model}`
- **Request correlation:** every upstream LLM call gets an internal `request_id` (UUID). Retries preserve the same `request_id` but increment `attempt`. All logs include `{provider, model, request_id, attempt, status}`. Without this, retry logs become soup.

### Step 1.4 — `ModelCatalog` (provider + capabilities, not just IDs)
In `agent-core`:
```rust
pub struct ModelSpec {
    pub id: ModelId,
    pub provider: ProviderKind,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_vision: bool,
    pub default_for_plan: bool,
}

pub trait ModelCatalog: Send + Sync {
    fn resolve_allowed(&self, plan: &Plan, requested: Option<&str>) -> Result<&ModelSpec, ModelError>;
    fn default_for(&self, plan: &Plan) -> &ModelSpec;
}
```
- In `build_ctx`, replace `req.model.unwrap_or("claude-opus-4-7")` with `state.model_catalog.resolve_allowed(&tenant.plan, req.model.as_deref())?`.
- Reject `req.stream = true` when `!spec.supports_streaming`.
- **Execution order (do not reorder):**
  1. Resolve `ModelSpec` from catalog (handles aliases, plan gating).
  2. Build a **lightweight** `ToolRoutingDecision` from request metadata + router classification — cheap, no tool-definition loading yet.
  3. Gate on `spec.supports_tools`:
     - If `!spec.supports_tools && decision.tool_required` → reject per rules below.
     - If `!spec.supports_tools && !decision.tool_required` → skip full tool-definition loading entirely and force `tools = []`.
     - If `spec.supports_tools` → load definitions for `decision.selected_tools` normally.
  This avoids loading tool schemas the model can never invoke.
- **Tool routing decision (concrete type, not vibes):** routing returns
  ```rust
  pub struct ToolRoutingDecision {
      pub selected_tools: Vec<ToolDefinition>,
      pub tool_required: bool,
      pub reason: Option<ToolRequirementReason>,
  }
  pub enum ToolRequirementReason {
      ForcedCapability,
      ExternalStateRequired,    // mutation or retrieval outside the prompt
      AttachmentOrWorkspaceOp,  // request carries attachments / workspace actions
  }
  ```
  `tool_required = true` **only** when one of those three reasons applies. Low semantic confidence alone must NOT set `tool_required`. The coding agent must not invent additional reasons.
- **Non-tool-model rules** (precise — do not paraphrase):
  - If `!spec.supports_tools` and `decision.tool_required == false`, **skip capability routing and force `tools = []`**. Do not reject.
  - If `!spec.supports_tools` and `req.forced_capability` is set (`reason == ForcedCapability`), return `HttpError::validation("model", "selected model does not support tools")`.
  - If `!spec.supports_tools` and `decision.tool_required == true` for any other reason, return `HttpError::validation("model", "task requires tools; selected model is text-only")`.
- **Model alias compatibility:** `ModelCatalog` must accept currently-shipping model ID strings as aliases that resolve to canonical `ModelSpec`s. Emit `model_alias_used{alias,target}` on every alias hit. Do not remove aliases for at least one release cycle. (If the model surface is internal-only with zero external clients, document that here and skip aliases.) **Required test:** unit test that a known legacy model string resolves to the expected canonical `ModelSpec` AND emits the `model_alias_used` metric exactly once.
- Reject vision attachments when `!spec.supports_vision`.
- **Input token enforcement strategy:**
  - Use a provider-aware tokenizer where available; otherwise a conservative estimator (e.g. `chars / 3.5` for Latin scripts, lower for CJK).
  - Fail closed only when the estimate exceeds `spec.max_input_tokens` by a configured safety margin (default 10%).
  - Emit metric `llm_input_token_estimate_exceeded{provider,model}`.
  - Exact token enforcement is a follow-up; estimator must not block this phase.

### Step 1.5 — Equalize metering between blocking and streaming
- Extract `agent::metering::record_agent_usage(state, tenant_id, model, input_tokens, output_tokens, tool_calls, duration_ms)` emitting `AgentTurn` + `Token` usage events and updating quota.
- Call from blocking handler (~[agent.rs:743](apps/backend/crates/agent-gateway/src/routes/agent.rs#L743)) and streaming handler (~[agent.rs:1413](apps/backend/crates/agent-gateway/src/routes/agent.rs#L1413)).
- Integration test: same prompt to `/agent` and `/agent/stream` → identical `usage_events` rows.

### Step 1.6 — Tool result size cap before re-feed
- After every tool invocation, truncate `content` to a configurable max (default 32 KB). On truncation append `\n…[truncated N bytes]`, keep `is_error: false`, record `tool_result_truncated{tool}` metric.
- Apply uniformly in blocking + streaming paths (will be consolidated in Phase 2).

### Step 1.7 — Cleanup on `delete_node` + reindex on `restore_version` (using existing mechanisms)
File: [workspaces.rs:678](apps/backend/crates/agent-gateway/src/routes/workspaces.rs#L678).
- **Capture a `DeletePlan` before the store deletes anything.** If the store cascades and does not return descendants, the cleanup list is lost. New types in `agent-core`:
  ```rust
  pub struct DeletePlan { pub nodes: Vec<DeletedWorkspaceNodeRef> }
  pub struct DeletedWorkspaceNodeRef {
      pub id: Ulid,
      pub kind: NodeKind,
      pub virtual_path: String,
      pub object_key: Option<String>,
  }
  ```
  Sequence:
  ```rust
  let plan = workspace_store.plan_delete(&tenant.tenant_id, id).await?;
  workspace_store.delete_node(&tenant.tenant_id, id).await?;
  cleanup_after_delete(&state, &tenant.tenant_id, &plan).await; // best-effort
  ```
- `cleanup_after_delete` runs vector + content cleanup for **every node in the plan** (root + descendants) as `tokio::join!` (**not `try_join!`** — best-effort semantics require running both even if one fails). Prefer `node.object_key` for content cleanup when present (post-Step 3.4 world); fall back to `virtual_path` otherwise. This keeps the helper correct across the storage migration without rewriting it later:
  ```rust
  for node in &plan.nodes {
      let content_key = node.object_key.as_deref().unwrap_or(node.virtual_path.as_str());
      let (vec_res, content_res) = tokio::join!(
          state.vector_store.delete_by_node_id(tenant_id, node.id),
          state.workspace_content.delete_all_versions(tenant_id, content_key),
      );
      if let Err(e) = vec_res { tracing::error!(error=%e, node_id=%node.id, "vector cleanup failed"); }
      if let Err(e) = content_res { tracing::error!(error=%e, node_id=%node.id, "content cleanup failed"); }
  }
  ```
  Never fail the API response on cleanup error.
- `restore_version`: after writing restored content, call the **same indexing path** as `patch_content` (the synchronous one for now — Phase 3 makes it durable).

### Phase 1 gate

```
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm test:e2e:web
```
Plus: manually exercise presign upload/download via [apps/web](apps/web) chat attachment flow.

---

## Phase 2 — Agent runtime extraction (agent.rs only)

No workspace/storage changes in this phase. One concern per phase.

### Step 2.1 — Module skeleton (re-export, no logic move)
Create `apps/backend/crates/agent-gateway/src/agent/` with empty modules that re-export from `routes::agent`. Keep `routes/agent.rs` compiling. This is the seam.
```
agent/mod.rs
agent/context.rs
agent/tool_execution.rs
agent/persistence.rs
agent/streaming.rs
agent/metering.rs
agent/provider/mod.rs
agent/provider/anthropic.rs
```
**Module direction rule:** the `agent::* → routes::agent` reverse dependency is permitted **only during Step 2.1** as a transitional seam. By Step 2.7 the direction must be inverted: `routes::agent` may depend on `agent::*`, and `agent::*` must not import anything from `routes::*`. Phase 2 gate must verify this with an xtask or grep-based static check (not `cargo deny` — that is for licenses/advisories). Concretely: a check that fails CI if `apps/backend/crates/agent-gateway/src/agent/**/*.rs` contains `crate::routes` or `super::routes`, with an allowlist entry only during Step 2.1.

### Step 2.2 — Provider request/response types (boundary only)
Define `ProviderRequest`, `ProviderResponse`, `ProviderEvent`, `ProviderError` in `agent/provider/mod.rs`. **Internal only** — Anthropic JSON shaping stays in `agent/provider/anthropic.rs`. Routes still use current types.

### Step 2.3 — Extract `execute_tool_calls`
Move tool-execution loop body (currently duplicated in blocking + streaming) into `agent/tool_execution.rs::execute_tool_calls(ctx, calls) -> Vec<ToolResult>`. Both paths call the same function. Behavior must be **event-sequence equivalent** — verified via typed `AgentEvent` assertions in the Step 0.2 harness, not via byte/string comparison of serialized SSE output. JSON field ordering may shift without breaking semantics; the test must care about event identity and order, not bytes.

### Step 2.4 — Extract persistence + metering hooks
- `agent/persistence.rs::persist_final_message(ctx, text)` and `set_title_if_first(ctx)`.
- `agent/metering.rs` already exists from Step 1.5; ensure both runners use it.
- Indexing stays synchronous here; Phase 3 makes it durable.

### Step 2.5 — `AgentTurnRunner` with async, cancellation-aware sink
```rust
#[async_trait]
pub trait AgentEventSink: Send {
    async fn emit(&mut self, ev: AgentEvent) -> Result<(), AgentEmitError>;
}

pub struct AgentTurnRunner { /* state, tenant, ctx */ }

impl AgentTurnRunner {
    pub async fn run(&mut self, sink: &mut dyn AgentEventSink, cancel: CancellationToken)
        -> Result<(), AgentError>;
}
```
- **Keep events typed end-to-end.** `AgentTurnRunner` emits `AgentEvent`; encoding to wire format lives only at the sink boundary.
  - `BlockingSink` converts `AgentEvent` → final JSON response.
  - `SseSink` converts `AgentEvent` → `axum::response::sse::Event` via `mpsc::Sender<Result<sse::Event, Infallible>>`.
  - **Do not** push `Bytes` into the sink — tests then become string matching and event semantics leak across the boundary.
- **Cancellation:** if the SSE client disconnects (sink emit returns `AgentEmitError::ClientGone` or `cancel` token fires), stop the loop **before** the next tool call. Do not burn tokens for a ghost tab.
- Blocking + streaming route handlers become ~10 lines each.

### Step 2.6 — Typed message/content/tool model
Replace `serde_json::Value` plumbing in `AgentCtx`:
```rust
pub struct AgentMessage { pub role: MessageRole, pub content: MessageContent }
pub enum MessageContent { Text(String), Blocks(Vec<ContentBlock>) }
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: ToolName, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
}
```
Conversion to Anthropic JSON lives only in `agent/provider/anthropic.rs`. Migrate call sites incrementally with a `Value`-based shim during migration.

### Step 2.7 — Native `AnthropicProvider`
Move all Anthropic HTTP + SSE parsing from `routes/agent.rs` and `agent/streaming.rs` into `agent/provider/anthropic.rs` implementing `trait AgentProvider` (defined here, used in Phase 4). `AgentTurnRunner` no longer references `reqwest` directly.

### Step 2.8 — Replace `std::sync::Mutex` in async paths
- Swap `state.onboarding_guards: Mutex<HashMap<...>>` for `dashmap::DashMap<String, Arc<tokio::sync::Mutex<()>>>`. Remove `.lock().unwrap()` in handlers.
- Audit `state.registry.lock().unwrap()` — replace with `arc_swap::ArcSwap<RegistrySnapshot>`; reads via `Guard`, writes publish a new snapshot.

### Step 2.9 — Enable agent tenant isolation tests
Flip Step 0.4 agent-related tests from `#[ignore]` to active.

### Phase 2 gate
Same as Phase 1 gate.

---

## Phase 3 — Workspace/storage hardening + indexing callers

> Scope note: this phase is primarily `workspaces.rs`, but Step 3.2 also touches `agent.rs` indexing call sites — they must migrate to the same durable job to avoid two indexing pathways. Do not pretend this phase is workspace-only.

### Step 3.1 — Workspace module split
```
workspace/mod.rs
workspace/access.rs           ← effective_user_id, get_accessible_node helpers
workspace/content_indexing.rs ← reindex_node()
workspace/presign.rs
workspace/versioning.rs
workspace/errors.rs
```
`routes/workspaces.rs` becomes thin handler wiring (~250 lines).

### Step 3.2 — Durable workspace indexing jobs with version guards (replaces workspace AND agent indexing callers)
Land as three sub-commits:

**3.2a** — Add `jobs::WorkspaceIndexJob { tenant_id, node_id, content_version }` variant + worker to the [jobs crate](apps/backend/crates/jobs). Worker logic below.

**3.2b** — Replace `tokio::spawn` indexing in workspace `patch_content` / `restore_version` with `state.jobs.enqueue(WorkspaceIndexJob{…})`.

**3.2c** — Replace `tokio::spawn` indexing at [agent.rs:715](apps/backend/crates/agent-gateway/src/routes/agent.rs#L715) and [agent.rs:1159](apps/backend/crates/agent-gateway/src/routes/agent.rs#L1159) with the same enqueue. This sub-commit depends on the `AgentTurnRunner` persistence hook from Step 2.4 existing.

- **Worker logic (mandatory):**
  - Load current `content_version` for the node.
  - If `job.content_version < current_version` → record `workspace_index_skipped_stale` metric and return success without upsert.
  - Embeddings keyed by `(node_id, content_version)`.
  - After successful upsert of new version, delete embeddings for older versions. **If the vector store supports transactions, do upsert+delete transactionally; otherwise perform ordered/idempotent operations and require all search queries to filter by latest `content_version` so stale embeddings are harmless until cleanup completes.** Qdrant does not provide multi-operation transactions — do not pretend otherwise.
- Retries: exponential backoff, max 5 attempts. Persist `index_status` (`pending|indexing|ok|failed|skipped_stale`) on the node.
- Metrics: `workspace_index_failures`, `workspace_index_skipped_stale`, `workspace_index_duration_ms`.

### Step 3.3 — Typed workspace store errors
- Define in `agent-core::store`:
  ```rust
  pub enum WorkspaceStoreError { Validation(String), NotFound, Forbidden, Conflict, Storage(StorageError) }
  ```
- Refactor store functions to return `Result<_, WorkspaceStoreError>` instead of `anyhow::Error`.
- Replace string-matching `map_err` at [workspaces.rs:77](apps/backend/crates/agent-gateway/src/routes/workspaces.rs#L77) with exhaustive `match`.

### Step 3.4 — Stable object-key migration: dual-read / dual-write (no flip yet)
This is migration work, not cleanup. Land each sub-step as its own commit.

1. **Schema:** add `object_key: Option<String>` column to workspace_nodes (nullable during migration).
2. **Dual-read:** `workspace_content::read` tries `node_id`-keyed path first, falls back to `virtual_path` key. Emit `workspace_content_read_fallback` metric on fallback.
3. **Dual-write with explicit failure semantics:** `workspace_content::write` writes to **both** the new key (`tenants/{tid}/nodes/{node_id}/content`) and the legacy `virtual_path` key. Rules:
   - The new `node_id` write is **primary**. If it fails, the request fails.
   - The legacy write is **best-effort**. Failure logs `workspace_content_legacy_write_failed` metric + `tracing::error!` but does NOT fail the request as long as the primary succeeded.
   - If the legacy write succeeded but the primary failed, attempt to delete the orphan legacy write (best-effort) before returning the error.
   - Reconciliation script verifies both keys exist for nodes written during the dual-write window and reports drift.

### Step 3.5 — Backfill + audit + cutover
1. **Backfill job:** scan all workspace_nodes; for each, copy legacy key → `node_id` key if missing. Idempotent. Resumable. Run as a `jobs::WorkspaceBackfillObjectKey` job.
2. **Audit script:** `apps/backend/xtask/src/audit_object_keys.rs` reports coverage % and lists nodes still falling back.
3. **Cutover gate:** require 100% coverage AND `workspace_content_read_fallback` metric at 0 for 24h before proceeding.
4. **Flip reads:** remove fallback path; read from `node_id` key only.
5. **Stop legacy writes:** remove dual-write; write only to `node_id` key.
6. **`move_node` simplification:** now only updates metadata row; no object copy required. Remove copy-then-delete logic at [workspaces.rs:484](apps/backend/crates/agent-gateway/src/routes/workspaces.rs#L484).
7. **GC:** background job deletes orphaned legacy `virtual_path` objects after a grace period (7 days).

### Step 3.6 — Enable workspace tenant isolation tests
Flip Step 0.4 workspace-related tests from `#[ignore]` to active. Add property tests (proptest) for `VirtualPath::is_same_or_within` and `VirtualPath::is_strict_child_of` covering: reflexivity rules (`is_same_or_within` true on equal, `is_strict_child_of` false on equal), sibling-prefix rejection (`/foo` does not contain `/foo-bar`), and traversal/encoded-segment rejection at `parse` time.

### Phase 3 gate
Same as Phase 1 + testcontainer-backed storage integration tests.

---

## Phase 4 — Provider abstraction; optional Rig adapter

> The goal is the **provider boundary**, not Rig adoption. Native Anthropic remains the default. Rig is an optional adapter, evaluated on concrete benefit.

### Step 4.1 — `AgentProvider` trait formalization
```rust
#[async_trait]
pub trait AgentProvider: Send + Sync {
    async fn complete(&self, req: ProviderRequest) -> Result<ProviderResponse, ProviderError>;
    async fn stream(
        &self,
        req: ProviderRequest,
        sink: &mut dyn ProviderEventSink,
        cancel: CancellationToken,
    ) -> Result<(), ProviderError>;
}
```
`AgentTurnRunner` selects provider via `ModelCatalog::resolve_allowed(...).provider`.

### Step 4.2 — Optional `RigProvider` behind cargo feature
- Add `rig` as optional dependency behind `feature = "rig-provider"`.
- Implement `RigProvider` using Rig's agent + dynamic-tool model.
- **Keep native `AnthropicProvider` as the default** — Rig's SSE format may drift, and Anthropic is our primary path.
- Add per-provider integration tests against recorded fixtures.

### Step 4.3 — Prompt hooks
```rust
#[async_trait]
pub trait PromptHook: Send + Sync {
    async fn before_turn(&self, ctx: &mut AgentCtx) -> Result<(), HookError>;
    async fn after_turn(&self, ctx: &AgentCtx, usage: &Usage) -> Result<(), HookError>;
}
```
Built-in hooks: `LogTokensHook`, `RedactPiiHook`, `EnforceMaxInputHook`. All token-monitoring (native + Rig) routes into `metering::record_agent_usage`.

**`RedactPiiHook` policy scope (do not get clever):**
- Logs / audit redaction: **on by default**.
- Prompt mutation: **opt-in per deployment**.
- Tool input mutation: **prohibited** unless the tool explicitly declares `redaction_safe_fields`.

Blind redaction of tool inputs will silently break legal-name lookups, email routing, workspace search, and billing references.

### Step 4.4 — Property + load tests
- **Property test** (proptest) for `AgentTurnRunner`: for any sequence of tool_use/tool_result pairs (bounded depth ≤ `max_tool_calls`), runner terminates and emits exactly one `done` event.
- **Cancellation test:** dropping the sink mid-stream stops tool execution within N ms.
- **Load test** (k6 against staging): 50 concurrent streaming sessions; assert p95 < target, no deadlocks, no panics.

### Phase 4 gate
Same as Phase 3 plus load test report.

---

## Target file layout (end state)

```
routes/agent.rs              ~150 lines (handler wiring)
agent/mod.rs
agent/context.rs
agent/runner.rs
agent/tool_execution.rs
agent/streaming.rs
agent/persistence.rs
agent/metering.rs
agent/provider/mod.rs
agent/provider/anthropic.rs
agent/provider/rig.rs        (behind feature)

routes/workspaces.rs         ~250 lines (handler wiring)
workspace/mod.rs
workspace/access.rs
workspace/content_indexing.rs
workspace/presign.rs
workspace/versioning.rs
workspace/errors.rs

agent-core/src/model_catalog.rs            ← Step 1.4
agent-core/src/workspace/virtual_path.rs   ← Step 1.1 (is_same_or_within, is_strict_child_of)
agent-core/src/store/workspace/errors.rs   ← Step 3.3
jobs/src/workspace_index.rs                ← Step 3.2
jobs/src/workspace_backfill.rs             ← Step 3.5
```

## Execution checklist

- [ ] **Phase 0** — Steps 0.1, 0.2, 0.3, 0.4 (no behavior change)
- [ ] **Phase 1** — Steps 1.1 → 1.7 (correctness + security)
- [ ] **Phase 2** — Steps 2.1 → 2.9 (agent runtime only)
- [ ] **Phase 3** — Steps 3.1 → 3.6 (workspace + storage migration only)
- [ ] **Phase 4** — Steps 4.1 → 4.4 (provider abstraction)

**Stop condition (scope creep guard):** if any step requires touching code outside its declared phase scope — e.g. a Phase 2 step needs to modify `workspaces.rs`, or a Phase 3 step needs to modify `agent.rs` outside the explicitly-scoped Step 3.2c call sites — **stop coding, write a short design note in the PR/commit describing the unplanned scope, and get explicit approval before continuing.** Shortcuts that quietly widen a phase are how this refactor turns into a crime scene.

## Notes for the executing agent

1. **Suggestion #1 (onboarding) is a false alarm caused by misnaming.** Step 0.1 renames; never invert the condition.
2. **One concern per phase.** Do not mix agent runtime work (Phase 2) with workspace/storage work (Phase 3). Exception: Step 3.2c migrates agent indexing call sites — explicitly scoped.
3. **Never use `str::starts_with` for path containment.** Use `VirtualPath::is_strict_child_of` for child uploads, `is_same_or_within` for content routes. The security boundary is `VirtualPath::parse` — constructors must be private.
4. **Never retry an LLM call after the first response byte.** See Step 1.3. Carry `request_id` across retries.
5. **Non-tool-model behavior:** for normal chat on a text-only model, force `tools = []`; do NOT reject. Only reject when tools are actually required (forced capability or tool-required task).
6. **Cancellation is a feature, not a polish item.** The async sink in Step 2.5 must propagate client-disconnect into the tool loop.
7. **Keep `AgentEvent` typed end-to-end.** Encoding to SSE/JSON happens only at sink boundary — never push `Bytes` into the runner.
8. **Module direction:** transitional reverse import allowed only in Step 2.1; by Step 2.7 `agent::*` must not import `routes::*`.
9. **Best-effort cleanup uses `tokio::join!`, not `try_join!`.** `try_join!` short-circuits on first error and defeats the purpose.
10. **Storage migration (Step 3.4–3.5) is dual-read/dual-write/backfill/cutover.** Never "copy keys and switch." Dual-write: new key is primary, legacy is best-effort.
11. **Indexing jobs must check `content_version` before upserting.** Stale-write races are the default failure mode otherwise.
12. **Targeted tests every commit; full e2e at phase boundaries.** Do not run `pnpm test:e2e:web` per micro-step — it gets skipped.
13. Preserve existing routing audit fields when refactoring; observability is already good — do not regress it.
14. When unsure whether to place logic in `agent-core` vs `agent-gateway`, prefer `agent-core` (testable without HTTP).
