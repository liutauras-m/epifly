# ConusAI Platform — Implementation Plan

**Based on:** Backend API Reference Review v0.1.0 + 2026 Best-Practice Elevation  
**Review Date:** 2026-05-04  
**Status:** Production-Ready Foundation (95% aligned)  
**Target:** 100% compliance — Rig v0.30 hooks, compile-safe OpenAPI, zero-trust security posture

---

## Overview

The backend is architecturally sound (Axum 0.8+ + Rig + Utoipa + Tower/Governor). This plan tightens existing phases, merges related work, and adds one Rig-modernization step (hook-based extensibility). All changes stay inside the documented project structure (`crates/`, `capabilities/`, `docs/`, `evals/`). No scope creep.

**Key upgrades over v1 plan:**
- Rig v0.30: `max_turns()` (renamed from `max_depth`/`max_steps`), `HookAction` / `ToolCallHookAction::Skip` for safe agent loops
- `thiserror` + `ApiError` enum (enhances existing `ConusAiError`) instead of ad-hoc `AppError` struct
- `axum-autoroute` for compile-time OpenAPI accuracy — zero handler/spec drift
- Generated TS/Zod from `/openapi.json` via `openapi-typescript` — no manual type duplication
- Phases 5+6 merged into a single reusable Tower auth+plan layer
- API keys stored as `BLAKE3(key)` hash — never plaintext

---

## Phase 0 — Prep: Dependency Alignment

**Goal:** Workspace `Cargo.toml` reflects 2026 stack. All agent calls use `max_turns()`.

**Estimated effort:** 1h / ~400 tokens

### Steps

1. **Update `apps/backend/Cargo.toml` workspace**
   - Set `edition = "2024"` on all crates
   - Bump: `rig-core = "0.30"`, `utoipa = "5"`, add `thiserror`, `axum-autoroute = "0.2"`

2. **Run `cargo update`** and resolve any version conflicts

3. **Rename all `max_depth`/`max_steps` agent builder calls to `max_turns()`** across:
   - `crates/agent-core/src/agent/`
   - `crates/agent-gateway/src/routes/agent.rs`
   - `crates/agent-gateway/src/routes/chat.rs`
   - `evals/src/runners/`

4. **Verify**: `cargo check --workspace`

---

## Phase 1 — P0: Unified Structured Error Envelope + Rig Error Mapping

**Goal:** Every HTTP response and MCP JSON-RPC body uses a single `{"error": {...}}` shape. Frontend type-narrows on one discriminant. Rig errors (`MaxTurnsError`, `ToolError`, `ProviderError`, `ExtractionError`) map cleanly.

**Estimated effort:** 3h / ~1200 tokens

### Steps

1. **Enhance `crates/common/src/error.rs`** — extend existing `ApiError` / `ConusAiError` using `thiserror`:
   ```rust
   use thiserror::Error;
   use serde::{Deserialize, Serialize};
   use utoipa::ToSchema;

   #[derive(Debug, Error, Serialize, Deserialize, ToSchema)]
   #[serde(rename_all = "snake_case")]
   pub enum ApiError {
       #[error("Authentication failed: {0}")]
       Authentication(String),
       #[error("Rate limit exceeded")]
       RateLimit { retry_after: Option<u64> },
       #[error("Not found: {resource}")]
       NotFound { resource: String },
       #[error("Validation error")]
       Validation { field: String, message: String },
       #[error("Rig agent error: {0}")]
       Agent(String), // maps RigError / MaxTurnsError / ExtractionError
       #[error("Internal error")]
       Internal { request_id: String, trace_id: Option<String> },
   }

   #[derive(Debug, Serialize, ToSchema)]
   pub struct ErrorEnvelope {
       pub error: ApiError,
   }

   impl IntoResponse for ApiError {
       fn into_response(self) -> Response {
           let status = match &self {
               ApiError::Authentication(_) => StatusCode::UNAUTHORIZED,
               ApiError::RateLimit { .. } => StatusCode::TOO_MANY_REQUESTS,
               ApiError::NotFound { .. } => StatusCode::NOT_FOUND,
               ApiError::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
               ApiError::Agent(_) | ApiError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
           };
           // always set X-Request-ID header + body
           (status, Json(ErrorEnvelope { error: self })).into_response()
       }
   }
   ```

2. **Add Rig error mapping helper** in `crates/agent-core/src/agent/runtime.rs`:
   ```rust
   pub fn map_rig_error(e: rig::error::RigError) -> ApiError { ... }
   ```

3. **Replace all ad-hoc error returns** in `crates/agent-gateway/src/routes/` using `?` returning `Result<_, ApiError>`:
   - `chat.rs`, `agent.rs`, `threads.rs`, `workspaces.rs`, `files.rs`, `capabilities.rs`, `audit.rs`
   - `mcp.rs` — still HTTP 200; embed JSON-RPC error codes in body per spec; rate-limit still 429

4. **Register in Utoipa components** in `main.rs`:
   ```rust
   #[openapi(components(schemas(ApiError, ErrorEnvelope, ...)))]
   ```

5. **Verify**: `cargo clippy -- -D warnings && cargo test -p agent-gateway -p common`

---

## Phase 2 — P0: Request-ID + Trace Propagation in All Error Paths

**Goal:** Every error response carries `X-Request-ID` header and `request_id` in body. Clients correlate errors with traces.

**Estimated effort:** 1.5h / ~600 tokens

### Steps

1. **Confirm `Extension<RequestId>` flows** from `crates/agent-gateway/src/mw/` to `ApiError::into_response`
   - Middleware already injects it on success — ensure error path reads the same extension

2. **Embed `request_id` in `ApiError::Internal`** (done in Phase 1) and set `X-Request-ID` response header on all `IntoResponse` paths

3. **Propagate `traceparent` context into Rig calls** using `opentelemetry::Context::with_remote_span_context`
   - Read `traceparent`/`tracestate` headers in middleware
   - Pass OTel context to every `.prompt().max_turns(n).send()` call

4. **Verify**: `curl -i http://localhost:8080/v1/chat/completions` with invalid token → confirm `X-Request-ID` header and `error.request_id` in body match

---

## Phase 3 — P1: Compile-Safe OpenAPI with `axum-autoroute`

**Goal:** `/openapi.json` is 100% accurate and usable for codegen. Zero drift between handlers and spec — unannotated handler = compile error.

**Estimated effort:** 4h / ~1800 tokens

### Steps

1. **Add to `crates/agent-gateway/Cargo.toml`**: `axum-autoroute = "0.2"`

2. **Annotate every handler** with `#[utoipa::path]`:
   - `POST /v1/chat/completions` — `request_body(content = ChatRequest)`, `responses(200, 401, 429, 500)`, SSE variant in description
   - `POST /v1/agent/completions` — `thread_id` in 200 schema, `tool_call_start`/`tool_call_result` delta events in description
   - All CRUD: threads, workspaces, files, audit, capabilities, MCP

3. **Add `SecurityScheme` definitions**:
   - `BearerAuth`: `http`, `bearer`, `JWT`
   - `CookieAuth`: `apiKey`, `cookie`, `conusai_session`
   - `ApiKeyAuth`: `apiKey`, `header`, `X-API-Key` (added in Phase 5+6)

4. **Document MCP tool schema format** — example `tools/list` response showing Rig `Tool` trait JSON schema fields

5. **Wire `axum-autoroute`** — compile-time completeness enforcement

6. **Verify**: `curl -sf http://localhost:8080/openapi.json | jq '.paths | keys | length'` — all routes present

---

## Phase 4 — P1: Document `chat` Reserved Fields

**Goal:** No silent data loss — callers know which fields `/v1/chat/completions` accepts but ignores.

**Estimated effort:** 1h / ~400 tokens

### Steps

1. **In `crates/agent-gateway/src/routes/chat.rs`**, add doc comments to `ChatRequest` reserved fields:
   ```rust
   /// Reserved for future agentic routing. Currently ignored by this endpoint.
   /// Use POST /v1/agent/completions if thread/workspace context is needed.
   pub thread_id: Option<String>,
   pub workspace_node_id: Option<String>,
   pub max_turns: Option<u32>,
   ```

2. **In Utoipa annotation** for `POST /v1/chat/completions`, set `description = "Reserved. Use /v1/agent/completions for full agentic context."` on these schema fields

3. **Update `docs/frontend/api.md`** Notes section to match code comments exactly

---

## Phase 5+6 — P2: Enterprise Auth + Plan Enforcement (Merged)

**Goal:** Single reusable Tower layer handles `X-API-Key` auth and per-plan limit clamping. Applied once in router — handlers stay thin.

**Estimated effort:** 4h / ~1500 tokens

### Steps

1. **Add `API_KEYS` config** to `crates/common/src/config/`
   - Format: comma-separated `BLAKE3(key):tenant_id:plan` tuples in `API_KEYS` env var
   - Store only the hash — validate by hashing the incoming `X-API-Key` value and comparing (never log the raw key)

2. **Create `crates/agent-gateway/src/mw/auth.rs`** — `ApiKeyExtractor` layer:
   - Check `X-API-Key` header before JWT check
   - On hash match: resolve `tenant_id` and `plan`; set `TenantClaims` extension identically to JWT path

3. **Create `crates/agent-gateway/src/mw/plan.rs`** — `PlanEnforcer` layer:
   - Read `TenantClaims` from extension
   - Reject if plan claim missing or unrecognized (prevents malformed JWTs bypassing limits)
   - Clamp `max_tokens` and `max_turns` to plan limits before reaching handler

4. **Apply layers once** in `protected_router()`:
   ```rust
   Router::new()
       .layer(ApiKeyExtractor::new(config.api_keys.clone()))
       .layer(PlanEnforcer::new())
   ```

5. **Remove per-handler clamping** — handlers trust middleware has enforced limits

6. **Add `ApiKeyAuth` security scheme** to OpenAPI spec (see Phase 3)

7. **Write unit tests** for Free/Pro/Enterprise boundaries:
   - `max_tokens` clamping, `max_turns` clamping, rate limit window

8. **Document in `docs/frontend/api.md`** under Authentication section

---

## Phase 7 — P3: Thread Auto-Title on Creation

**Goal:** Threads have human-readable titles for display in the sidebar Recents/History.

**Estimated effort:** 2h / ~700 tokens

### Steps

1. **On `POST /v1/threads`**, accept `first_message: Option<String>` in request body:
   - If provided: use first 60 characters as title (no LLM call for v1)
   - If not provided: default to `"New Thread <timestamp>"`

2. **On `POST /v1/agent/completions`**, after first assistant turn completes:
   - If thread was auto-created and title is still default: run fast Rig prompt (`max_turns: 1`, `max_tokens: 20`) to generate 5–8 word title
   - Alternatively use `rig::Extractor` for structured output
   - Persist via `thread_store.update_title(thread_id, title)`

3. **Confirm `title` is present in `GET /v1/threads` list response** (`ThreadSummary` shape)

---

## Phase 8 — P3: End-to-End Observability via Rig v0.30 Hooks

**Goal:** Unified tracing across all agent paths (chat, agent/completions, invoice pipeline, evals) without copy-paste. Uses Rig v0.30 `HookAction` / `ToolCallHookAction` pattern — SRP: hooks own cross-cutting concerns, handlers stay thin.

**Estimated effort:** 3h / ~1100 tokens

### Steps

1. **Implement `TracingHook` in `crates/agent-core/src/agent/`**:
   ```rust
   pub struct TracingHook;

   impl PromptHook<Anthropic> for TracingHook {
       async fn on_completion_call(&self, ctx: &HookContext) -> HookAction {
           // start OTel child span with tenant_id, model, thread_id, max_turns
           HookAction::Continue
       }
       async fn on_tool_call(&self, ctx: &ToolCallHookContext) -> ToolCallHookAction {
           // record tool_name, args in span event
           ToolCallHookAction::Continue
       }
   }
   ```

2. **Implement `PermissionHook`** — uses `ToolCallHookAction::Skip { reason }` to reject tools the current plan tier cannot call

3. **Attach hooks** in `AgentBuilder` in `agent-core`:
   ```rust
   GeneralAgentBuilder::new()
       .with_hook(TracingHook)
       .with_hook(PermissionHook::for_plan(plan))
       .max_turns(plan.max_turns())
   ```

4. **Add span attributes**: `tenant_id`, `plan`, `model`, `workspace_node_id`, `thread_id`, `tool_calls_made`, `max_turns`

5. **Verify** in OTLP collector (Jaeger or Tempo) that a complete trace chain appears: frontend → Axum handler → Rig turns → tool calls → Anthropic

---

## Phase 9 — P4: Generated Frontend Type Contracts

**Goal:** Single source of truth — TypeScript types and Zod schemas generated from `/openapi.json`. No manual duplication in `packages/types/`.

**Estimated effort:** 2.5h / ~900 tokens

### Steps

1. **Run `openapi-typescript`** against the completed `/openapi.json`:
   ```bash
   npx openapi-typescript http://localhost:8080/openapi.json -o packages/types/src/api.ts
   ```

2. **Add Zod schemas** for runtime validation (orval can generate these; or add manually for key shapes):
   - `ThreadSchema`, `WorkspaceNodeSchema`, `ErrorEnvelopeSchema`

3. **Wire into `apps/web`**:
   - `AgentCompletionRequest` → `useChat` / `useCompletion` body type
   - `ErrorEnvelope` → unified error display in `features/chat` and `features/agent`

4. **Export from `packages/types/src/index.ts`**

5. **Add codegen script** to root `package.json`:
   ```json
   "generate:types": "openapi-typescript http://localhost:8080/openapi.json -o packages/types/src/api.ts"
   ```

6. **Verify**: `bun x tsc --noEmit -p packages/types`

---

## Phase 10 — P4: Rich Capability Metadata per Tenant

**Goal:** Frontend adapts UI per tenant (hide image input, show available tools) based on capability metadata.

**Estimated effort:** 1.5h / ~500 tokens

### Steps

1. **Extend `CapabilityItem`** with optional fields:
   ```rust
   pub models: Vec<String>,          // e.g. ["claude-opus-4-7"]
   pub max_turns_limit: Option<u32>,
   pub supported_tools: Vec<String>,
   ```

2. **In capabilities handler**, resolve `models` from `ANTHROPIC_MODEL` env var + per-tenant config overrides

3. **Use existing Qdrant semantic search** for capability discovery — no new infrastructure needed

4. **Update Utoipa schema** and `docs/frontend/api.md` accordingly

---

## Verification Checklist (run after each phase)

```bash
# Backend
cargo clippy --workspace -- -D warnings
cargo test --workspace --all-features
cargo test -p agent-core   # Rig hooks
cargo test -p common       # ApiError shapes

# Error shape spot-check
curl -i http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer invalid" | grep -E "X-Request-ID|error"

# OpenAPI completeness
curl -sf http://localhost:8080/openapi.json | jq '.paths | keys | length'

# Frontend contracts
bun x tsc --noEmit -p packages/types
bun run lint   # biome

# Integration
turbo build
curl -sf http://localhost:8080/health
```

---

## Milestone Summary

| Phase | Priority | AI Time | Tokens (est.) | Outcome |
|-------|----------|---------|---------------|---------|
| 0 — Dependency alignment | Prep | 1h | 400 | Rig v0.30, `max_turns()`, edition 2024 |
| 1 — `ApiError` envelope + Rig mapping | P0 | 3h | 1200 | Single error shape + `thiserror` |
| 2 — Request-ID + trace in errors | P0 | 1.5h | 600 | Every error traceable |
| 3 — Compile-safe OpenAPI + autoroute | P1 | 4h | 1800 | Zero handler/spec drift |
| 4 — Document chat reserved fields | P1 | 1h | 400 | No silent data loss |
| 5+6 — Auth layer + plan enforcement | P2 | 4h | 1500 | `X-API-Key` hashed + reusable Tower layers |
| 7 — Thread auto-title | P3 | 2h | 700 | Human-readable thread names |
| 8 — Rig Hook-based tracing | P3 | 3h | 1100 | Pluggable, SRP-compliant observability |
| 9 — Generated TS/Zod contracts | P4 | 2.5h | 900 | No manual type duplication |
| 10 — Rich capability metadata | P4 | 1.5h | 500 | Adaptive per-tenant UI |
| **Total** | | **~24h** | **~9100** | |

> **Recommended starting point:** Phase 8 (Rig Hooks) first — gives pluggable, reusable behavior for every agent path (chat, agent/completions, invoice pipeline, evals) without copy-paste. Then Phase 1 (ApiError envelope) for consistent error handling.
