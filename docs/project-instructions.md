You are an expert AI-agents Rust developer (think: the team that built Claude, Claude CoWork, or similar production agent platforms). Your goal is to always suggest the best, newest approach to build highly maintainable and easily extensible agent software — applying SRP, clean code, and community-standard naming throughout.

**Rules of engagement:**
- Do not introduce unnecessary features or patterns.
- Always prefer community-canonical naming; rename boldly when a better name exists.
- Follow community project-structure best practices at every layer.
- Seek the newest, idiomatic practices; make code reusable, generic, and easy to maintain.
- Estimate effort in AI-hours and approximate token cost.
- Challenge every decision for consistency and quality.

**Key resource:** https://docs.rig.rs

---

## Canonical Names

| Recommended Name | Explanation |
|---|---|
| `Agent` | Core agent type. Clean, idiomatic Rust name following modern agent frameworks (LangGraph, CrewAI, LlamaIndex). Removes redundant "General" prefix. |
| `AgentBuilder` | Standard builder-pattern type for constructing `Agent` instances. |
| `CompletionProvider` | Provides model completions. More precise and future-proof than `LlmProvider`; aligns with Rig's existing `Completion*` APIs and industry terminology. |
| `CapabilityProvider` | Core abstraction for agent capabilities (replaces `ToolProvider`). Richer than tools — supports prompt chains, memory, sub-agents, permissions, and composite behaviors. |
| `CapabilityFactory` | Creates and registers capabilities. Consistent with the capability-centric architecture. |
| `PromptChainCapability` | Capability implemented via prompt chaining / LLM chains. Clear, descriptive, and self-documenting. |
| `CapabilityCard` | Registry metadata and introspection record for a capability (replaces `ToolCard`). |
| `CapabilityAdmin` | Administrative interface for managing the capability registry. |
| `CapabilityRegistry` | In-memory capability registry and loader. |
| `SemanticCapabilityRouter` | Pre-filters capabilities to top-K before an LLM turn. |
| `DynamicPromptCapability` | DB-backed versioned prompt capability. |
| `TraceReplayCapability` | Capability that turns recorded traces into replay plans. |
| `ArtifactBridge` | Materialises tool-produced artifacts into workspace nodes and object storage. |

---

## 1. Monorepo Layout

```
conusai-platform/
├── apps/
│   ├── backend/                   ← Rust backend workspace
│   ├── web/                       ← SvelteKit web frontend
│   └── browser-shell/             ← Browser-shell client
├── docs/                          ← Architecture docs, plan, verify scripts
├── docker-compose.yml             ← Profiles: infra | full | observability
├── Makefile
└── start.sh / stop.sh
```

> **Frontend note:** The workspace already contains `apps/web/` and `apps/browser-shell/`. The primary in-product UI remains the Foundry server-rendered UI built with Askama, served directly by `agent-gateway`.

## 2. Backend (`apps/backend/`)

### Cargo Workspace Members

- [crates/common](../apps/backend/crates/common) — Shared utilities, foundational types, `PromptTemplate`, error types
- [crates/agent-core](../apps/backend/crates/agent-core) — Agent runtime (`Agent`, `AgentBuilder`), capability registry, Rig integration
- [crates/agent-gateway](../apps/backend/crates/agent-gateway) — OpenAI-compatible HTTP gateway + Askama UI + Utoipa OpenAPI 3.1
- [evals](../apps/backend/evals) — Evaluation framework (runners + scorers)

### Key Workspace Dependencies

| Category | Crates |
|----------|--------|
| Async runtime | `tokio` (full), `tokio-stream` |
| AI / LLM | `rig-core` **0.36** (Anthropic; native SSE streaming via `CompletionModel::stream()`), `rig-qdrant` **0.2.5** |
| Database / Vector DB | `redb` 4 (embedded KV, postcard codec), `qdrant-client` 1 + `rig-qdrant` 0.2.5 (768-d cosine ANN) |
| HTTP server | `axum` 0.8 (ws, multipart), `tower` 0.5, `tower-http` 0.6 (cors, trace, br, fs) |
| HTTP client | `reqwest` **0.13** (json, stream) |
| Serialization | `serde`, `serde_json`, `toml` |
| Config | `figment` 0.10 (env, toml) |
| Errors | `thiserror` 2, `anyhow` |
| Observability | `tracing`, `tracing-subscriber`, `opentelemetry` 0.27 (metrics), `opentelemetry_sdk` 0.27 (rt-tokio, metrics), `opentelemetry-otlp` 0.27 (tonic, metrics), `opentelemetry-prometheus` 0.27, `prometheus` 0.13, `tracing-opentelemetry` 0.28 |
| WASM | `wasmtime` 44 (component-model), `wasmtime-wasi` 44 |
| Auth/Crypto | `jsonwebtoken` 9, `sha2` 0.10, `hmac` 0.12, `blake3` 1, `base64` 0.22 |
| Schema/validation | `schemars` 0.8 (derive) |
| OpenAPI | `utoipa` 5 (axum_extras, chrono, uuid, ulid), `utoipa-swagger-ui` 9 |
| Object storage | `object_store` 0.11 (aws/S3/MinIO) |
| Embeddings (optional) | `fastembed` **5** (feature-gated: `local-embeddings`) |
| Server-side UI | `askama` 0.12 (Foundry UI; server-rendered product surface) |
| IDs | `ulid` 1.1 (time-sortable, serde) |
| Utilities | `uuid`, `chrono`, `bytes`, `futures`, `async-trait`, `bon` 3, `clap` 4, `colored` 2 |

- **Rust edition:** 2024 · **Rust version:** 1.95 · **WASM target:** `wasm32-wasip1` · **rust-toolchain components:** `rustfmt`, `clippy`, `rust-src`, `rust-analyzer`

### API Routes

Three router groups — `public_router`, `protected_router` (tenant middleware), `admin_router` (super-admin JWT).

#### Public (no auth)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health + capability count |
| POST | `/v1/auth/login` | Issue JWT (dev: any creds; prod: `DEV_PASSWORD` env) |
| GET | `/openapi.json` | OpenAPI 3.1 spec (Utoipa-generated) |
| GET | `/docs` | Swagger UI |
| GET | `/metrics` | Prometheus text exposition (`/metrics`, no auth — restrict via network in prod) |

#### Protected (Bearer JWT or `X-API-Key`)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/chat/completions` | OpenAI-compatible chat, optional SSE stream |
| POST | `/v1/agent/completions` | Thread-aware agent completions with tool calls |
| GET | `/v1/capabilities` | List registered capabilities |
| GET | `/v1/capabilities/search` | Semantic capability search (Qdrant ANN) |
| POST | `/mcp` | MCP JSON-RPC 2.0 tool dispatch |
| POST | `/v1/files` | Multipart file upload (MinIO-backed) |
| GET | `/v1/files/{token}` | Token-gated file download (Bearer JWT required) |
| GET | `/v1/audit` | Audit event log |
| POST | `/v1/workspaces` | Create workspace node |
| GET | `/v1/workspaces/tree` | Workspace tree |
| GET | `/v1/workspaces/search` | Workspace search |
| GET | `/v1/workspaces/{id}` | Get node |
| DELETE | `/v1/workspaces/{id}` | Delete node |
| GET | `/v1/workspaces/{id}/content` | Get node content |
| PATCH | `/v1/workspaces/{id}/content` | Update node content |
| POST | `/v1/workspaces/{id}/move` | Move node |
| POST | `/v1/workspaces/{id}/share` | Share node |
| POST | `/v1/workspaces/{id}/unshare` | Unshare node |
| GET | `/v1/tasks` | List background task statuses |
| GET | `/v1/tasks/{id}` | Get single task status |
| GET | `/v1/tasks/{id}/sse` | SSE stream for task lifecycle events |
| GET | `/v1/threads/{id}/messages` | Paginated message list for a thread |
| GET | `/api/realtime/workspace` | WebSocket — workspace change event stream |
| GET | `/v1/shells/{device_id}/control` | Browser-shell WebSocket control channel |

#### Super-admin (`role=super_admin` JWT)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/admin/capabilities` | List all capabilities |
| POST | `/admin/capabilities` | Register new capability |
| POST | `/admin/capabilities/reload` | Reload all capabilities |
| POST | `/admin/capabilities/register` | Register remote MCP capability service |
| POST | `/admin/capabilities/validate` | Validate capability manifests |
| POST | `/admin/capabilities/test` | Test-invoke a capability |
| GET | `/admin/capabilities/{name}` | Get single capability |
| GET | `/admin/capabilities/{name}/manifest` | Get raw manifest |
| PATCH | `/admin/capabilities/{name}` | Update capability |
| PATCH | `/admin/capabilities/{name}/enabled` | Enable/disable capability |
| DELETE | `/admin/capabilities/{name}` | Delete capability |
| POST | `/admin/capabilities/{name}/reload` | Reload single capability |
| PUT | `/admin/capabilities/{name}/prompt` | Create a new dynamic prompt version |
| GET | `/admin/capabilities/{name}/prompt?version=N` | Get dynamic prompt (latest or pinned) |
| GET | `/admin/capabilities/{name}/prompt/versions` | List all prompt versions |
| GET | `/admin/capabilities/namespaces?prefix=X` | Browse namespace tree |
| GET | `/admin/jobs` | List all registered jobs |
| GET | `/admin/jobs/{name}` | Get single job summary |
| POST | `/admin/jobs/{name}/run` | Enqueue a background job immediately |
| GET | `/admin/tasks` | List all task statuses (admin view) |
| GET | `/admin/devices` | List browser-shell device tokens |
| POST | `/admin/devices` | Issue a browser-shell device token |
| DELETE | `/admin/devices/{id}` | Revoke a browser-shell device token |

### CORS

`build_cors()` in `main.rs` reads `WEB_ORIGIN` env (comma-separated origins, default `http://localhost:3000`) and configures `tower-http` `CorsLayer` with explicit methods, headers (`Authorization`, `Content-Type`, `X-Tenant-Id`, `X-API-Key`), exposed header `X-Request-Id`, and `allow_credentials: true`. Never uses `CorsLayer::permissive()` in production.

## Architecture Decisions

- [ADR 0003 - Unified Postgres + CocoIndex](docs/adr/0003-unified-postgres-cocoindex.md) *(superseded by ADR 0009)*
- [ADR 0004 - Semantic Capability Router & Dynamic Prompts](docs/adr/0004-semantic-capability-router-and-dynamic-prompts.md)
- [ADR 006 - Tauri Browser Shell](docs/adr/006-tauri-browser-shell.md)
- [ADR 007 - Capability Module Rename](docs/adr/007-capability-module-rename.md)
- [ADR 008 - Multi-Platform Shell](docs/adr/008-multi-platform-shell.md)
- [ADR 009 - redb + Qdrant + RustFS](docs/adr/0009-redb-qdrant-rustfs.md)

## v0.3.2 New Concepts

### Semantic Capability Router

`SemanticCapabilityRouter` replaces "send all enabled tools to the LLM". At every agent turn it:
1. Embeds the user query.
2. ANN-searches `capability_embeddings` (Qdrant, cosine) with namespace + tag filters.
3. Returns the top-K (default 20, max 50) providers whose distance ≤ 0.65.
4. Results are moka-cached for 60 s (blake3 key = tenant + query + config).

Wire into `AgentBuilder`: `builder.with_semantic_router(router)`.

### Namespaces

`ToolManifest.namespace` is a dot-separated slug (`erp.po`, `accounting.gl`). Use `NamespaceFilter` variants (`Any`, `Exact`, `Prefix`, `AnyOf`) for routing and payload filtering. Validated by `RegisteredToolValidator::validate_namespace`.

### Dynamic Prompts (`ToolKind::DynamicPrompt`)

Capabilities backed by versioned rows in `dynamic_prompts`. Push new versions via `PUT /admin/capabilities/{name}/prompt` without a deploy. The factory is `DynamicPromptFactory`; the provider is `DynamicPromptCapability`.

### Bulk Capability-Spec Factory

`CapabilitySpecFactory` implements `BulkCapabilityFactory`. Call `registry.run_bulk_load()` at boot to stream all enabled rows from `capability_specs` (redb) and embed them in 256-row batches. Hot-reload via `RealtimeService::subscribe_capability_spec_changes()` backed by in-process tokio broadcast channels.

### Artifact Bridge

`ArtifactBridge` materialises `ToolOutput.artifacts` into RustFS objects and workspace nodes after tool execution. Treat it as part of the agent loop, not as a capability concern.

### Tower Quota Middleware

`RouterQuotaLayer` on `POST /v1/agent/completions` injects `RouterQuotaConfig` into request extensions. Read `max_tools_per_turn` and `max_invokes_per_turn` from extensions in the agent handler to enforce hard caps. Configured via `CONUSAI_MAX_TOOLS_PER_TURN` / `CONUSAI_MAX_INVOKES_PER_TURN` env vars.

### Browser Shell

Browser-shell device registration is gated by `CONUSAI_FEATURE_BROWSER_SHELL=1` and `PLATFORM_ADMIN_TOKEN`. The WebSocket control channel lives at `GET /v1/shells/{device_id}/control` and uses the browser-shell device token for validation.

