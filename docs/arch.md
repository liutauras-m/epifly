# ConusAI Platform — Architecture & Functionality

> **v0.3.2** — Semantic capability router, namespace + tag partitioning, dynamic prompts, bulk capability-spec factory, OTel GenAI metrics, Tower quota middleware. All persistent stores remain Postgres-only.

A production-grade multitenant AI agent platform. The monorepo contains a **Rust + Rig** backend (`apps/backend/`) and WASM/MCP capabilities. The built-in **Foundry UI** (served by the gateway at `GET /`) provides workspace management, agent chat, file upload, and invoice extraction without a separate frontend app.

---

## v0.3.1 Additions

### Postgres-backed stores (replacing in-memory stubs)

All three persistent stores are Postgres-backed via `sqlx`:

| Store | Trait | Backend table(s) |
|---|---|---|
| `PostgresThreadStore` | `ThreadStore` | `threads`, `messages` |
| `PostgresWorkspaceStore` | `WorkspaceStore` | `workspace_nodes`, `content_embeddings` |
| `PostgresAuditStore` | `AuditStore` | `audit_events` |

A `PgVectorStore` (`agent_core::vector_store`) backs semantic capability search using the `capability_embeddings` table with a DiskANN vector index (cosine distance, 1536 dims).

### Workspace indexer (`agent_core::indexing`)

| File | Purpose |
|---|---|
| `coco_indexer.rs` | `WorkspaceIndexer` — crawls workspace filesystem, chunks content, generates embeddings, upserts to `content_embeddings` via pgvector |
| `embedding_service.rs` | `EmbeddingService` trait; `OpenAiEmbeddingService` (default, `text-embedding-3-small`, 1536 dims); `NoopEmbeddingService` (test mode) |
| `local_embedding_service.rs` | `LocalEmbeddingService` — feature-gated (`local-embeddings`), uses `fastembed` 5 for on-device embeddings |
| `real_fs_watcher.rs` | `RealFsWatcher` — watches filesystem for changes, triggers re-indexing at configurable intervals |

On startup: if `WORKSPACES_ROOT` is set, `main.rs` spawns an initial index pass then starts `RealFsWatcher`. Embedding backend selected via `EMBEDDING_BACKEND` env (`local` → `LocalEmbeddingService`, `openai` → `OpenAiEmbeddingService`, default → `NoopEmbeddingService`).

### Realtime service (`agent_core::realtime`)

`RealtimeService` — broadcast service for workspace change events via WebSocket. `WorkspaceChangeEvent` is fanned out to connected subscribers. Exposed at `GET /api/realtime/workspace` (Bearer JWT protected).

### WASM / wasmtime 44

- Target: `wasm32-wasip1` (per `rust-toolchain.toml`)
- `WasmToolLoader` wraps wasmtime 44 API

### New `AppState` fields (v0.3.1)

| Field | Type | Purpose |
|---|---|---|
| `pool` | `Option<PgPool>` | Shared Postgres connection pool |
| `embedding_service` | `Arc<dyn EmbeddingService>` | Embedding backend for indexing and search |
| `vector_store` | `Arc<PgVectorStore>` | Postgres pgvector ANN store |
| `realtime_service` | `Option<Arc<RealtimeService>>` | WebSocket broadcast service |

### New REST endpoint (v0.3.1)

| Method | Path | Auth | Description |
|---|---|---|---|
| `GET` | `/api/realtime/workspace` | Bearer JWT | WebSocket — workspace change event stream |
| `GET` | `/v1/threads/{id}/messages` | Bearer JWT | Retrieve messages for a thread |

---

## v0.3.2 Additions

### Semantic Capability Router (`agent_core::tools::semantic_router`)

```
User message
    │
    ▼
SemanticCapabilityRouter::select(query, tenant)
    │  ┌─────────────────────────────────────┐
    │  │ 1. blake3 cache-key lookup (moka)   │
    │  │ 2. embed query (EmbeddingService)   │
    │  │ 3. ANN search (PgVectorStore top-K) │
    │  │ 4. namespace + tag filter           │
    │  │ 5. distance threshold (≤ 0.65)      │
    │  │ 6. include_always overrides         │
    │  └─────────────────────────────────────┘
    │
    ▼
Vec<Arc<dyn CapabilityProvider>>  (top-K, ≤ 50)
    │
    ▼
Tool definitions → Anthropic / LLM
```

Key types:
| Symbol | Location | Purpose |
|---|---|---|
| `SemanticCapabilityRouter` | `tools/semantic_router.rs` | Core router; cache + ANN |
| `SemanticRouterConfig` | same | top_k, max_distance, namespace, tags_any, cache_ttl |
| `RouterMetrics` | same | Atomic counters (cache_hits, total_selects, etc.) |
| `NamespaceFilter` | `tools/namespace.rs` | Any / Exact / Prefix / AnyOf |

### Namespace & Tag Partitioning

`ToolManifest` now has `namespace: Option<String>` (dot-separated, e.g. `erp.po`) and `tags: Vec<String>`. The `ToolRegistry` maintains a hierarchical `namespace_index: HashMap<String, Vec<String>>` for fast admin autocomplete (`namespace_children(prefix)`).

Validator enforces: dot-separated ASCII slug segments `[a-z][a-z0-9_]*`, ≤ 6 segments, empty string = unnamespaced.

**New DB columns:**
- `capability_embeddings.namespace TEXT NOT NULL DEFAULT ''`
- `capability_embeddings.tags TEXT[] NOT NULL DEFAULT '{}'`
- Indexes: `cap_embed_ns_idx`, `cap_embed_tags_idx` (GIN)

Migration: `20260507000000_capability_namespaces.up.sql`

### Dynamic Prompts (`agent_core::chains::dynamic_prompt`)

DB-backed versioned prompt storage. `ToolKind::DynamicPrompt` capabilities load their `LlmChainConfig` (model, prompt_template, system_prompt, etc.) from the `dynamic_prompts` table at runtime. Supports:
- `load_latest()` — fetches highest version row
- `with_pinned_version(n)` — pins to specific version
- `invalidate()` — clears moka cache entry
- 60s TTL moka cache per capability name + version

Migration: `20260507000100_dynamic_prompts.up.sql`

**New admin endpoints:**
| Method | Path | Description |
|---|---|---|
| `PUT` | `/admin/capabilities/{name}/prompt` | Upsert prompt version |
| `GET` | `/admin/capabilities/{name}/prompt?version=N` | Fetch prompt (latest or pinned) |
| `GET` | `/admin/capabilities/{name}/prompt/versions` | List all versions |
| `GET` | `/admin/capabilities/namespaces?prefix=X` | Browse namespace tree |

### Bulk Capability-Spec Factory (`agent_core::tools::providers::capability_spec`)

`CapabilitySpecFactory` implements `BulkCapabilityFactory` — streams rows from `capability_specs` in chunks of 256, batch-embeds descriptions, upserts to `capability_embeddings`, and registers `CapabilityProvider` instances into `ToolRegistry`. Domain partitioning is via `namespace` (e.g. `erp.po`, `crm.lead`, `accounting.gl`); the factory itself is domain-neutral. Supports hot-reload via Postgres `LISTEN capability_specs_changed` (trigger on all mutations).

`ToolRegistry` gains:
- `register_bulk_factory(factory)` — register a `BulkCapabilityFactory`
- `run_bulk_load()` — run all registered factories (boot time)

Migration: `20260507000200_capability_specs.up.sql`

### OTel GenAI Metrics

New metrics in `common::metrics`:
| Metric | Type | Description |
|---|---|---|
| `gen_ai.semantic_router.cache_hit` | Counter | Cache hits |
| `gen_ai.semantic_router.top_k` | Histogram | Capabilities selected per turn |
| `gen_ai.semantic_router.distance` | Histogram | Cosine distance of top-1 hit |
| `gen_ai.tool.calls` | Counter | Total tool calls dispatched |
| `capability_router_select_seconds` | Histogram | Router select latency |
| `capability_invoke_seconds` | Histogram | Capability invocation latency |

### Tower Quota Middleware (`agent_gateway::mw::router_quota`)

`RouterQuotaLayer` wraps the `/v1/agent/completions` route, injecting `RouterQuotaConfig` into request extensions for per-turn tool/invoke budget enforcement.

Env vars: `CONUSAI_MAX_TOOLS_PER_TURN` (default 25), `CONUSAI_MAX_INVOKES_PER_TURN` (default 10).

### New `AppState` fields (v0.3.2)

| Field | Type | Purpose |
|---|---|---|
| `semantic_router` | `Arc<SemanticCapabilityRouter>` | Pre-filters tools to top-K per turn |

### Realtime Capability-Spec Hot-reload

`RealtimeService::subscribe_capability_spec_changes()` returns an unbounded channel receiver for `(namespace, tool_name)` tuples. The caller spawns a task that calls `CapabilitySpecFactory::reload_one(registry, ns, tool_name)` on each notification.

---

## v0.3 Additions (2026-05-05)

| Concept | Canonical name | Location |
|---|---|---|
| Cron-driven job trait | `ScheduledJob` | `jobs::ScheduledJob` |
| On-demand async job trait | `BackgroundJob` | `jobs::BackgroundJob` |
| In-memory task tracker | `JobExecutor` | `jobs::JobExecutor` |
| Task lifecycle event | `TaskEvent` | `jobs::TaskEvent` |
| Task status snapshot | `TaskStatus` | `jobs::TaskStatus` |
| Job + executor registry | `JobRegistry` | `jobs::JobRegistry` |
| Cron scheduler service | `JobSchedulerService` | `jobs::JobSchedulerService` |
| Admin facade | `JobAdmin` | `jobs::JobAdmin` |
| Video transcription | `TranscribeVideoCapability` | `agent_gateway::capabilities::TranscribeVideoCapability` |

### New REST endpoints (v0.3)

| Method | Path | Auth | Description |
|---|---|---|---|
| `GET` | `/v1/tasks` | tenant JWT | List background task statuses |
| `GET` | `/v1/tasks/{id}` | tenant JWT | Get a single task status |
| `GET` | `/v1/tasks/{id}/sse` | tenant JWT | SSE stream for task lifecycle events |
| `GET` | `/admin/jobs` | super_admin JWT | List all registered jobs |
| `GET` | `/admin/jobs/{name}` | super_admin JWT | Get single job summary |
| `POST` | `/admin/jobs/{name}/run` | super_admin JWT | Enqueue a background job immediately |
| `GET` | `/admin/tasks` | super_admin JWT | List all task statuses (admin view) |

---

## v0.2 Breaking Renames

| Old name (v0.1) | New canonical name (v0.2) | Location |
|---|---|---|
| `GeneralAgent` | `Agent` | `agent_core::Agent` |
| `GeneralAgentBuilder` | `AgentBuilder` | `agent_core::AgentBuilder` |
| `LlmProvider` | `CompletionProvider` | `agent_core::CompletionProvider` |
| `ToolProvider` | `CapabilityProvider` | `agent_core::tools::provider::CapabilityProvider` |
| `ToolProviderFactory` | `CapabilityFactory` | `agent_core::tools::provider::CapabilityFactory` |
| `LlmChainTool` | `PromptChainCapability` | `agent_core::PromptChainCapability` |
| `RegisteredToolCard` / `ToolCard` | `CapabilityCard` | `agent_core::CapabilityCard` |
| `RegisteredToolAdmin` | `CapabilityAdmin` | `agent_core::CapabilityAdmin` |

**Askama v0.x UI decision:** the Foundry server-rendered UI remains Askama for the v0.x series. A Next.js frontend is an optional future addon that can consume existing `/v1/*`, SSE, and MCP endpoints.

---

## 1. Technology Stack

| Layer | Technology | Version |
|---|---|---|
| Language | Rust | 1.88 (stable) |
| Edition | Rust 2024 | — |
| WASM target | `wasm32-wasip1` | — |
| AI framework | `rig-core` | 0.36 |
| Rig Postgres | `rig-postgres` | 0.2.5 |
| HTTP framework | `axum` | 0.8 (+ `axum-extra` 0.10) |
| Async runtime | `tokio` | 1 (full features) |
| Vector DB | Postgres + pgvector / DiskANN (via `sqlx`) | pg17 + pgvector |
| Object storage | `object_store` | 0.11 (aws feature) |
| WASM runtime | `wasmtime` + `wasmtime-wasi` | 44 |
| Embeddings (optional) | `fastembed` | 5 (feature-gated: `local-embeddings`) |
| Auth | `jsonwebtoken` | 9 |
| Templates | `askama` | 0.12 |
| OpenAPI | `utoipa` + `utoipa-swagger-ui` | 5 / 9 |
| Observability | `opentelemetry` + OTLP + Prometheus | 0.27 |
| **Scheduled jobs** | **`tokio-cron-scheduler`** | **0.13** |
| Configuration | `figment` | 0.10 |
| Hashing | `sha2` 0.10, `blake3` 1, `hmac` 0.12 | — |
| Builder patterns | `bon` | 3 |
| IDs | `ulid` 1.1, `uuid` 1 | — |
| Serde | `serde` 1, `serde_json` 1, `toml` 0.8 | — |
| Schema validation | `schemars` 0.8 | — |
| Encoding | `base64` 0.22 | — |
| Error handling | `thiserror` 2, `anyhow` 1 | — |
| Futures | `futures` 0.3, `tokio-stream` 0.1 | — |

---

## 2. Repository Layout

```
conusai-platform/
├── docker-compose.yml          # postgres, minio, gateway, jaeger, otel-collector
├── Makefile
├── start.sh / stop.sh          # orchestration helpers
├── apps/
│   └── backend/                # Rust workspace root
│       ├── Cargo.toml          # workspace definition + all shared deps
│       ├── Dockerfile          # 4-stage cargo-chef image
│       ├── rust-toolchain.toml
│       ├── crates/
│       │   ├── common/         # shared types, error, telemetry, memory traits
│       │   ├── agent-core/     # agent runtime, LLM abstraction, tool registry, chains, memory impls
│       │   ├── jobs/           # ScheduledJob / BackgroundJob traits, JobExecutor, JobSchedulerService
│       │   └── agent-gateway/  # Axum HTTP gateway, UI, OpenAPI docs
│       ├── evals/              # evaluation framework (invoice + OCR suites)
│       └── capabilities/       # zero-code TOML capability definitions
└── docs/
    ├── arch.md                 # this document
    ├── ui-design.md
    ├── verify/verify.md
    └── adr/
        └── 005-workspace-access-control.md
```

---

## 3. Infrastructure

### docker-compose.yml (v3.9)

| Service | Image | Ports | Profiles | Purpose |
|---|---|---|---|---|
| `postgres` | `timescale/timescaledb-ha:pg17` | 5432 | infra, full | Primary DB — threads, workspaces, audit, pgvector embeddings |
| `minio` | `quay.io/minio/minio:RELEASE.2025-04-22T22-12-26Z` | 9000 (S3), 9001 (Console) | infra, full | S3-compatible object storage |
| `minio-init` | (same) | — | full | Creates bucket `conusai` on first start |
| `agent-gateway` | (built locally) | 8080 | full | HTTP API + UI gateway |
| `jaeger` | `jaegertracing/all-in-one:1.58` | 16686 (UI), 14317 (OTLP) | observability, full | Distributed trace UI |
| `otel-collector` | `otel/opentelemetry-collector-contrib:0.123.0` | 4317 (gRPC), 4318 (HTTP) | observability, full | OTLP receiver / Jaeger exporter |

- **Profiles:** `infra` (core services), `full` (everything), `observability` (tracing stack).
- **Volumes:** `postgres_data:/var/lib/postgresql/data`, `minio_data:/data`, `./capabilities:/app/capabilities:ro`, `./workspaces:/app/workspaces:rw`.
- **MinIO dev creds:** `minioadmin` / `minioadmin`.
- All services declare healthchecks; `agent-gateway` depends on `postgres` (healthy) and `minio-init` (completed).

### Dockerfile — 4-stage cargo-chef build

| Stage | Base | Purpose |
|---|---|---|
| `planner` | `rust:1.88-slim` | `cargo chef prepare` → `recipe.json` |
| `cacher` | `rust:1.88-slim` | `cargo chef cook --release` (dependency layer cache) |
| `builder` | `rust:1.88-slim` | Full `cargo build --release --bin agent-gateway` |
| `gateway` | `debian:bookworm-slim` | Stripped runtime image with binary + assets |

Runtime image: `libssl3`, `ca-certificates`, `curl`; exposes 8080; HEALTHCHECK via `curl /health`.

### rust-toolchain.toml

```toml
channel = "stable"               # Rust 1.88
targets = ["wasm32-wasip1"]      # WASM capability builds
components = ["rustfmt", "clippy", "rust-src", "rust-analyzer"]
```

### Documentation files

| File | Purpose |
|---|---|
| [docs/ui-design.md](ui-design.md) | Design tokens (colour, type, spacing, motion) and component recipes. |
| [docs/verify/verify.md](verify/verify.md) | End-to-end verification plan — JWT helpers, curl recipes (Phases 0–14). |
| [docs/adr/005-workspace-access-control.md](adr/005-workspace-access-control.md) | ADR for private-by-default + selective-sharing ACL model. |

---

## 4. Crates

### 4.1 `crates/common` — Shared Utilities

**Purpose:** foundational types and newtypes, unified error hierarchy, HTTP error envelope, telemetry bootstrap, MCP JSON-RPC 2.0 types, WASM loader, layered config, path safety, audit log trait, memory store traits + in-memory implementations, OpenTelemetry metric helpers.

**Key dependencies:** `tokio`, `serde`/`serde_json`, `figment`, `thiserror`/`anyhow`, `tracing`, `tracing-subscriber`, `opentelemetry` 0.27, `opentelemetry_sdk`, `opentelemetry-otlp`, `opentelemetry-prometheus`, `prometheus`, `tracing-opentelemetry`, `wasmtime` 44, `reqwest` 0.13, `uuid`, `chrono`, `ulid`, `async-trait`, `axum`, `utoipa`.

| File | Purpose |
|---|---|
| `src/lib.rs` | Re-exports all modules; `prelude` with `Result`, `ConusAiError`. |
| `src/types.rs` | Typed ID newtypes — ULID-backed `ThreadId`, `NodeId`; string-backed `TenantId`, `UserId`, `ToolId`. All `serde(transparent)`. |
| `src/error.rs` | `ConusAiError` enum: `Config`, `Tool`, `Wasm`, `WasmRuntime(String)`, `Mcp`, `Rig(String)`, `Storage`, `Validation`, `NotFound`, `Api { status, message }`, `Io`, `Other`. HTTP error envelope: `ErrorEnvelope { error: ApiErrorBody }` + `ApiErrorKind` discriminated union (`Authentication`, `RateLimit { retry_after }`, `NotFound`, `Validation { field }`, `Agent`, `Internal { request_id }`). `HttpError` builder with `IntoResponse`. All schemas registered via `utoipa::ToSchema`. |
| `src/config/mod.rs` | `AppConfig { server, capabilities_dir, telemetry, llm }`. `LlmConfig { default, aliases: HashMap<String, LlmAliasConfig>, providers: LlmProvidersConfig }`. `AnthropicProviderConfig { api_key_env, base_url, api_version }`. Default aliases: `opus → anthropic/claude-opus-4-7`, `haiku → anthropic/claude-haiku-4-5-20251001`. Loaded via `figment` (TOML + env override with `CONUSAI_` prefix). |
| `src/telemetry.rs` | `TelemetryGuard` (RAII shutdown). `init(service_name, log_level) -> (TelemetryGuard, prometheus::Registry)`. JSON `tracing-subscriber` + optional OTLP trace/metrics export. Single `SdkMeterProvider` with Prometheus + OTLP `PeriodicReader` (avoids duplicate-registry panic). |
| `src/http_client.rs` | `build_client()` → `reqwest::Client` (60 s timeout, UA `conusai-platform/0.1`). |
| `src/mcp.rs` | `JsonRpcRequest` / `JsonRpcResponse` / `JsonRpcError` (JSON-RPC 2.0). |
| `src/wasm.rs` | `WasmLoader` wrapping `wasmtime::Engine`: `load_bytes`, `load_file`, `new_store`. |
| `src/db.rs` | `create_pool(database_url) -> PgPool`. `PostgresPool` type alias for `sqlx::PgPool`. Shared Postgres connection pool construction. |
| `src/limits.rs` | `MAX_PROMPT_TOKENS=128k`, `MAX_RESPONSE_TOKENS=16k`, `MAX_CAPABILITY_SIZE_BYTES=50 MB`, `MAX_WASM_SIZE_BYTES=10 MB`, `REQUEST_TIMEOUT_SECS=120`, `MAX_CONCURRENT_AGENTS=64`, `MAX_MESSAGES_PER_THREAD=10_000`, `MAX_MESSAGES_BEFORE_SUMMARY=50`. |
| `src/path_safety.rs` | `safe_join(root, rel)` — rejects path components containing `..`. `join_under_tenant(root, tenant_id, rel)`. |
| `src/audit.rs` | `AuditEvent { id (ULID), tenant_id, timestamp, action, tool?, status, duration_ms?, metadata }`. `AuditStore` async trait: `append`, `list(tenant, limit)`. |
| `src/metrics.rs` | OTel metric definitions: `tool_invocations`, `tool_errors`, `tool_duration_ms` on meter `conusai.agent`; `storage_duration_ms`, `storage_errors` on meter `conusai.storage`; `llm_requests`, `llm_input_tokens`, `llm_output_tokens`. `record_error(span, err)`. `kv(k, v)` convenience constructor. |
| `src/memory/thread.rs` | `Thread { id (ThreadId/ULID), tenant_id, title, created_at, last_active, message_count, summary, metadata }`. `Message { role, content, tool_calls, timestamp, seq }`. `ToolCall { id, name, input, output }`. |
| `src/memory/workspace.rs` | `NodeKind { Folder, Conversation, File }`. `WorkspaceNode { id (ULID), tenant_id, owner_id, parent_id, kind, name, virtual_path, last_modified, shared_with, metadata }`. Helpers: `new_folder`, `new_conversation`, `validate_name` (rejects empty/>255/`/`/`\`/`..`/leading `.`; enforces `.md` for conversations), `join_virtual_path`, `effective_user_id` (maps `None` → `"__dev__"`). |
| `src/memory/store.rs` | `ThreadStore` trait: `create`, `get`, `messages`, `append`, `list(after cursor)`, `set_summary`, `set_title`. `WorkspaceStore` trait: `create_folder`, `create_conversation`, `list_accessible_children`, `get_accessible_node`, `get_ancestors`, `move_node`, `delete_node`, `share_node`, `unshare_node`, `bump_last_modified`, `search_nodes`, `index_content`, `bind_thread`. `WorkspaceContentStore` trait: `read`, `write`, `delete`. All take `tenant_id: &str` + `user_id: &str`. |
| `src/memory/inmem.rs` | `InMemoryThreadStore`, `InMemoryWorkspaceStore`, `InMemoryWorkspaceContent`, `InMemoryAuditStore` — zero-dependency `Mutex<HashMap<…>>` implementations. Full ACL, recursive delete, substring search. Activated via `CONUSAI_TEST_MODE=1`. |

**Tests (22):** path-traversal rejection, safe joins, MCP serialization, `ApiError` fields, limit invariants, thread/message/tool-call serde roundtrips, `WorkspaceNode` serde, every `validate_name` branch, `join_virtual_path`, `effective_user_id`.

---

### 4.2 `crates/agent-core` — Agent Runtime

**Purpose:** LLM abstraction layer, Rig integration, tool registry + discovery, tool execution (MCP, WASM, chain, native), tenant context, conversation service, invoice/contract/OCR pipelines, Postgres-backed stores (thread, workspace, audit, vector), MinIO workspace content, workspace indexer + embedding service, realtime broadcast service, workspace context builder, prompt templating, agent hooks, super-admin capability CRUD.

**Key dependencies:** all `common` deps + `rig-core` 0.36, `rig-postgres` 0.2.5, `schemars` 0.8, `base64` 0.22, `sha2`, `blake3`, `bon` 3, `object_store` 0.11.

#### LLM abstraction layer (`src/llm/`)

Single source of truth for all model access. No route, chain, or memory module constructs a provider client directly.

| File | Purpose |
|---|---|
| `llm/types.rs` | `LlmRequest` (builder via `bon`): `model`, `messages: Vec<rig::Message>`, `temperature`, `max_tokens`, `tools: Vec<Value>`, `tenant: Option<TenantId>`. `LlmResponse { content, usage: Option<LlmUsage>, finish_reason }`. `LlmUsage { input_tokens, output_tokens }`. `LlmChunk { delta, finish_reason }`. `LlmStream = Pin<Box<dyn Stream<Item=Result<LlmChunk, LlmError>> + Send>>`. `LlmBinding { provider: String, model: String }`. |
| `llm/error.rs` | `LlmError`: `Config(String)`, `Request(String)`, `Response(String)`, `UnknownAlias { alias }`, `ProviderNotFound { name }`, `Streaming(String)`. |
| `llm/provider.rs` | `CompletionProvider` async trait (dyn-safe via `async_trait`): `name()`, `supports_tools()` (default true), `supports_vision()` (default false), `supports_streaming()` (default true), `complete(req) -> Result<LlmResponse>`, `stream(req) -> Result<LlmStream>`. |
| `llm/registry.rs` | `LlmRegistry { providers, aliases, default: LlmBinding }`. Resolution order in `resolve_binding(alias_or_model, tenant)`: (1) `tenant.preferred_model`, (2) caller-supplied alias/model, (3) `tenant.plan.default_alias()`, (4) `self.default`. `from_config(LlmConfig, providers_map)`. `verify_llm_providers` validates all aliases at startup. |
| `llm/providers/anthropic.rs` | `AnthropicProvider { client: rig::providers::anthropic::Client }`. `from_env()` reads `ANTHROPIC_API_KEY`. `supports_vision() = true`. `stream()` uses Rig 0.36 native SSE (`CompletionModel::stream`) mapping `StreamedAssistantContent::Text` → `LlmChunk`. All calls `#[instrument]`. |
| `llm/streaming.rs` | OpenAI-compatible SSE stream helpers. |

#### Agent subsystem (`src/agent/`)

| File | Purpose |
|---|---|
| `agent/builder.rs` | `AgentBuilder` — fluent: `model`, `preamble`, `max_tokens`, `with_tenant`, `build`. Enforces `plan.max_tokens()` cap. `build_for_tenant` convenience constructor. `Agent::prompt(text)` attaches `TracingHook` + `max_turns` from plan, calls `rig::Agent::prompt(...).max_turns(...).with_hook(...)`. |
| `agent/hooks.rs` | `TracingHook { tenant_id, plan, thread_id }` implements `rig::agent::PromptHook<M>`. Emits `info!` on `on_completion_call` and `on_tool_call`. `PermissionHook` — future extension point for ACL checks. |
| `agent/runtime.rs` | `AgentRuntime { agent: Agent, registry: ToolRegistry }`. `for_tenant(model, preamble, registry, tenant)`. `run(input)`. `map_rig_error(msg)` — pattern-matches Rig error messages to typed `HttpError` variants. |

#### Context subsystem (`src/context/`)

| File | Purpose |
|---|---|
| `context/tenant.rs` | `UserRole { User, Admin, SuperAdmin }` (default `User`). `PlanTier { Free, Pro, Enterprise }` with `max_tokens()` (4k/16k/128k), `max_turns()` (3/8/20), `rate_limit_rpm()` (10/60/600), `default_alias()` (`haiku`/`opus`/`opus`). `TenantContext { tenant_id, user_id, plan, role, workspace_root, preferred_model? }` with `tenant_root()`, `safe_path(rel)`, `storage_prefix()`, `system_prompt()`, `span_fields()`. `TenantClaims { sub, tenant_id, plan, role, exp }` for JWT decode. |
| `context/conversation.rs` | `ConversationService` async trait: `create(tenant, node_id?)`, `append_message`, `load_history`, `resolve_for_node` (lazy thread binding), `list(tenant, limit, after)`, `get`. `DefaultConversationService { thread_store, workspace_store }` — coordinates thread create + `WorkspaceStore::bind_thread`. |
| `context/mod.rs` | `ConversationContext { id: Uuid, system_prompt?, history: Vec<HistoryEntry> }` with `push_user`, `push_assistant`, `to_rig_messages()`. |

#### Tools subsystem (`src/tools/`)

| File | Purpose |
|---|---|
| `tools/manifest.rs` | `ToolManifest { name, version, description, kind, tools: Vec<ToolDef>, config, tags, chain: Option<LlmChainConfig> }`. `ToolKind { Mcp, Wasm, Chain, Docker, Native }` (`serde rename_all = "lowercase"`). `ToolDef { name, description, input_schema }`. `LlmChainConfig { model, system_prompt?, prompt_template, vision, max_tokens, output_schema? }`. `from_toml`, `from_file`, `embedding_text`. |
| `tools/card.rs` | `CapabilityCard { id: Uuid, manifest, source_dir: PathBuf, embedding_id?, enabled, last_error?, registered_at, updated_at, provider: Option<Arc<dyn CapabilityProvider>> }`. |
| `tools/provider.rs` | `CapabilityProvider` async trait: `manifest()`, `invoke(tool_name, input, tenant) -> Result<Value>`, `tool_definitions()` (default impl — Anthropic format), `invoke_typed<I,O>` (default impl). `CapabilityFactory` trait: `supports(kind, name) -> bool`, `create(card) -> Result<Arc<dyn CapabilityProvider>>`. `invoke_typed_dyn` for `&dyn CapabilityProvider` callers. |
| `tools/registry.rs` | `ToolRegistry { cards, factories }`. `with_default_factories(llm)` pre-registers `McpFactory`, `WasmFactory`, `ChainFactory(llm)`, `BuiltinFactory`. Lifecycle: `register`, `unregister`, `replace`, `set_enabled`, `reload_capability(dir)`. Queries: `get`, `get_provider`, `all`, `all_enabled`, `search_by_tag`. `load_from_dir(dir)`. |
| `tools/discovery.rs` | `ToolDiscovery::from_env()` reads `CONUSAI_CAPABILITIES_DIR` (default `./capabilities`). `discover_into(&mut registry)`. |
| `tools/store.rs` | `RegisteredToolState { enabled, created_at, updated_at }`. `RegisteredToolStore` trait: `list`, `read_manifest`, `write_manifest`, `write_wasm`, `read_state`, `write_state`, `delete`, `capability_dir`. `FilesystemStore` — atomic writes via `.tmp` → rename. |
| `tools/validator.rs` | `RegisteredToolValidator`: `validate_name` (regex `^[a-z0-9-]{2,64}$`), `validate_manifest`. `ValidationReport { errors: Vec<RegisteredToolValidationError>, warnings: Vec<String> }`. |
| `tools/admin.rs` | `CapabilityAdmin` — coordinates `FilesystemStore` + `ToolRegistry` + `RegisteredToolValidator` + `AuditStore`. Methods: `list`, `get`, `get_manifest`, `create`, `update`, `set_enabled`, `delete`, `reload_one`, `reload_all`, `test_invoke`. `AdminLimits { max_capabilities: 64, max_manifest_bytes: 65536, max_wasm_bytes: 8 MiB }` (env-overridable). `build_admin(registry, audit_store)` factory. |
| `tools/executor.rs` | `ToolExecutor::invoke(registry, cap_name, tool_name, input, tenant)`. `#[instrument]` span: `tool.cap`, `tool.name`, `tenant_id`, `error.type`. Metrics: `tool_invocations`, `tool_duration_ms`, `tool_errors`. |
| `tools/mcp_adapter.rs` | `McpAdapter` — JSON-RPC 2.0 HTTP client: `call`, `list_tools`, `call_tool`. |
| `tools/wasm_loader.rs` | `WasmToolLoader` (wraps `wasmtime::Engine`). `load(card)`, `invoke_tool(card, tool, input)`. |
| `tools/providers/chain.rs` | `InvoiceProvider`, `ContractProvider`, `OcrProvider` — thin adapters to `*Pipeline` structs. `PromptChainCapability` path for data-driven manifests with `[chain]` block. `ChainFactory::new(llm)`. |
| `tools/providers/mcp.rs` | `McpProvider` + `McpFactory`. |
| `tools/providers/wasm.rs` | `WasmProvider` + `WasmFactory`. |
| `tools/providers/builtin.rs` | `BuiltinProvider` + `BuiltinFactory`. Routes `read_file`/`write_file`/`run_cargo` to `builtin/{fs,cargo}`. |
| `tools/builtin/fs.rs` | `read_file` / `write_file` — tenant-scoped via `safe_join`. Uses `tokio::fs`. |
| `tools/builtin/cargo.rs` | `run_cargo` — allowlisted subcommands (`check`, `test`, `build`, `clippy`, `fmt`) via `tokio::process::Command`; returns `{stdout, stderr, exit_code}`. |
| `tools/builtin/card.rs` | `builtin_tool_card()` — `CapabilityCard` with `kind: Native` and full JSON schemas for `read_file`, `write_file`, `run_cargo`. |

#### Chains (`src/chains/`)

| File | Purpose |
|---|---|
| `chains/extraction.rs` | `ExtractionPipeline` async trait: `model_id()`, `system_prompt()`, `run(bytes: Vec<u8>, tenant?) -> Result<Output>`. Default: `extract_from_bytes`, `extract_as_value`. Dyn-compatible. |
| `chains/invoice.rs` | `InvoiceLineItem`, `InvoiceData` (~20 fields, `JsonSchema`). `InvoicePipeline::new()` (default `claude-opus-4-7`), `with_model`, `with_tenant`. Private `run_extraction` — base64, Claude vision, strict JSON schema prompt, strip markdown fences, parse. Implements `ExtractionPipeline`. |
| `chains/contract.rs` | `ContractParty`, `ContractData`. `ContractPipeline` — same structure as invoice. |
| `chains/llm_chain.rs` | `PromptChainCapability { manifest, cfg: LlmChainConfig, prompt: PromptTemplate, llm: Arc<LlmRegistry> }`. `invoke` renders `prompt_template` with `{{input.*}}` / `{{tenant.*}}` via `PromptTemplate`, calls `LlmRegistry::resolve` + provider `complete`, optionally validates against `output_schema`. Enables zero-code `kind=chain` capabilities from TOML alone. |

#### Prompt subsystem (`src/prompt/`)

The `PromptTemplate` type lives in `common::prompt::template` and is re-exported from `agent_core::prompt`. This allows cross-crate reuse without a direct `agent-core` dependency.

| File | Purpose |
|---|---|
| `common/src/prompt/template.rs` | `PromptTemplate` — lightweight `{{key.subkey}}` mustache-like interpolation over `serde_json::Value` context. Dot-separated path resolution; missing paths → empty string. No external template engine dependency. |
| `agent-core/src/prompt/mod.rs` | Re-exports `common::prompt::PromptTemplate` for backwards-compatible import paths. |

#### Memory subsystem (`src/memory/`)

| File | Purpose |
|---|---|
| `memory/postgres_thread_store.rs` | `PostgresThreadStore` implements `ThreadStore`. Tables: `threads`, `messages`. Background auto-summarisation when `message_count % MAX_MESSAGES_BEFORE_SUMMARY == 0`. All methods `#[instrument]`. |
| `memory/postgres_workspace_store.rs` | `PostgresWorkspaceStore` implements `WorkspaceStore`. Tables: `workspace_nodes`, `content_embeddings`. Access filter: `tenant_id = X AND (owner_id = U OR shared_with @> ARRAY[U])`. `search_nodes` uses full-text + substring fallback. `index_content` stores content chunks as embeddings. |
| `memory/postgres_audit_store.rs` | `PostgresAuditStore` implements `AuditStore`. Table: `audit_events`. `list` returns newest-first with optional cursor. |
| `memory/minio_workspace_content.rs` | `MinioWorkspaceContent` implements `WorkspaceContentStore` via `Arc<dyn ObjectStore>`. Keys: `tenants/{tenant_id}/workspaces/{virtual_path}`. `read` returns `""` on `NotFound`. `delete` is best-effort. |
| `memory/context_builder.rs` | `ContextBuilder { store, content, truncator: Arc<dyn ContextTruncator> }`. `build_for_node(tenant, node_id, max_chars)` — walks ancestors, loads `CONTEXT.md` / `README.md` from MinIO per folder, loads conversation body; joins with `\n\n---\n\n`; delegates truncation to injected `ContextTruncator`; prefixes `# Workspace context\n`. `with_truncator(t)` builder for custom strategies. Never errors hard. Used by `routes/agent.rs` with `max_chars = 6000`. |
| `memory/truncator.rs` | `ContextTruncator` strategy trait: `truncate(sections, max_chars)`. `OldestFirstTruncator` (default) — removes sections from the front until budget fits; preserves last section. Pluggable: inject any `Arc<dyn ContextTruncator>` for alternate RAG policies. |

#### Indexing subsystem (`src/indexing/`)

| File | Purpose |
|---|---|
| `indexing/coco_indexer.rs` | `WorkspaceIndexer` — crawls workspace filesystem, chunks content, generates embeddings, upserts to `content_embeddings` table with pgvector |
| `indexing/embedding_service.rs` | `EmbeddingService` trait; `OpenAiEmbeddingService` (default, `text-embedding-3-small`, EMBEDDING_DIMS=1536); `NoopEmbeddingService` (test mode) |
| `indexing/local_embedding_service.rs` | `LocalEmbeddingService` — feature-gated (`local-embeddings`), uses `fastembed` 5 crate for on-device inference |
| `indexing/real_fs_watcher.rs` | `RealFsWatcher` — watches filesystem for changes, triggers incremental re-indexing |

#### Realtime subsystem (`src/realtime/`)

| File | Purpose |
|---|---|
| `realtime/` | `RealtimeService` — tokio broadcast channel service for `WorkspaceChangeEvent`. Gateway holds `Option<Arc<RealtimeService>>`; `None` in test mode. |

#### Vector store (`src/vector_store/`)

| File | Purpose |
|---|---|
| `vector_store/postgres.rs` | `PgVectorStore` — cosine ANN search over `capability_embeddings` and `content_embeddings` tables via direct `sqlx` queries. `CapabilityHit`, `ContentHit` result types. `PgVectorStore::new(pool)` / `PgVectorStore::noop()` (test mode). `vec_to_pg(v)` serialises `f32` slice to Postgres vector literal. |

**Public re-exports** (via `lib.rs`): `Agent`, `AgentBuilder`, `TracingHook`, `PermissionHook`, `map_rig_error`, `ContractData`, `ContractParty`, `ContractPipeline`, `ExtractionPipeline`, `InvoiceData`, `InvoiceLineItem`, `InvoicePipeline`, `PromptChainCapability`, `ConversationService`, `DefaultConversationService`, `PlanTier`, `TenantClaims`, `TenantContext`, `UserRole`, `CapabilityCard`, `ContextBuilder`, `ContextTruncator`, `OldestFirstTruncator`, `MinioWorkspaceContent`, `PostgresAuditStore`, `PostgresThreadStore`, `PostgresWorkspaceStore`, `PgVectorStore`, `EmbeddingService`, `WorkspaceIndexer`, `RealtimeService`, `AdminLimits`, `CapabilitySummary`, `CreateCapabilityRequest`, `CapabilityAdmin`, `TestInvokeRequest`, `TestInvokeResponse`, `UpdateCapabilityRequest`, `build_admin`, `builtin_tool_card`, `ToolDiscovery`, `CapabilityFactory`, `ToolRegistry`, `FilesystemStore`, `RegisteredToolState`, `RegisteredToolStore`, `RegisteredToolValidationError`, `RegisteredToolValidator`, `ValidationReport`, `LlmBinding`, `LlmChunk`, `LlmError`, `CompletionProvider`, `LlmRegistry`, `LlmRequest`, `LlmResponse`, `LlmStream`, `LlmUsage`.

**Tests (8):** registry register/get/tag-search; manifest embedding text; nonexistent-dir handling; WASM `ping` execution; `PostgresThreadStore` pool construction + query; `PgVectorStore::noop()` returns empty results.

---

### 4.3 `crates/jobs` — Scheduled + Background Job Infrastructure

**Purpose:** Provides two traits (`ScheduledJob` for cron-driven jobs and `BackgroundJob` for on-demand async tasks), an in-memory executor with `TaskStatus` tracking, SSE-ready broadcast channels, a `JobSchedulerService` backed by `tokio-cron-scheduler`, and a `JobAdmin` facade consumed by the gateway admin routes.

**Key dependencies:** `common` (for `AuditStore`), `tokio`, `tokio-cron-scheduler` 0.13, `async-trait`, `uuid`, `chrono`, `reqwest` (multipart for Whisper API calls).

**Dependency design:** `jobs` depends only on `common`, NOT on `agent-core`. This avoids a circular dependency (`agent-core` → `jobs` → `agent-core`). Gateway-level capabilities that need both (`TranscribeVideoCapability`) live in `agent-gateway`.

| File | Purpose |
|---|---|
| `src/job.rs` | `TaskState { Queued, Running, Completed, Failed }`. `TaskStatus { id, job_name, state, created/updated_at, result?, error? }`. `ScheduledJob` async trait: `name`, `cron`, `enabled` (default `true`), `run(ctx)`. `BackgroundJob` async trait: `name`, `run(input, ctx)`. |
| `src/context.rs` | `JobContext { audit_store, minio_endpoint?, bucket? }`. Cheap to `Clone` (all `Arc`). Shared across all job invocations. |
| `src/registry.rs` | `JobRegistry { scheduled: Vec<Arc<dyn ScheduledJob>>, background: HashMap<String, Arc<dyn BackgroundJob>>, ctx }`. `register_scheduled`, `register_background`. |
| `src/scheduler.rs` | `JobSchedulerService::start(registry)` — iterates `registered_jobs()`, creates a `tokio_cron_scheduler::Job` per enabled job, wires `Arc<JobContext>` into each async closure, starts scheduler. |
| `src/executor.rs` | `JobExecutor { tasks: RwLock<HashMap<Uuid, TaskStatus>>, channels: RwLock<HashMap<Uuid, Sender<TaskEvent>>> }`. `enqueue(job_name, input)` → `task_id`; spawns `tokio::spawn` that calls `BackgroundJob::run`, updates state, broadcasts `TaskEvent`. `get_status`, `list_tasks`, `subscribe` (SSE). |
| `src/admin.rs` | `JobAdmin { registry, executor }`. `list_jobs() -> Vec<JobSummary>`, `get_job(name)`, `run_now(name, input) -> Uuid`, `list_tasks(limit)`, `get_task(id)`. |
| `src/jobs/capability_health_check.rs` | `CapabilityHealthCheckJob` — cron `"0 */5 * * * *"`. Pings MinIO `/minio/health/live`. Logs warnings on failure. |
| `src/jobs/audit_log_cleanup.rs` | `AuditLogCleanupJob` — cron `"0 0 2 * * *"`. Reads `AUDIT_RETENTION_DAYS` (default 30). Placeholder — logs intent; full `delete_before` trait method is a future PR. |
| `src/jobs/video_transcription.rs` | `VideoTranscriptionJob` — downloads file from MinIO, calls OpenAI Whisper API (`OPENAI_API_KEY`), or returns a placeholder transcript. Output: `{ file_id, tenant_id, transcript, chars }`. |

**Tests (4):** echo job completes with result, fail job records error message, unknown job returns `Err`, `list_tasks` returns all enqueued task IDs.

---

### 4.4 `crates/agent-gateway` — HTTP API + Foundry UI

**Purpose:** OpenAI-compatible chat/agent endpoints, tool calling, MCP dispatch, capability search, file upload/download, JWT + API key + session auth, rate limiting, plan enforcement, request-ID correlation, Prometheus metrics endpoint, OpenAPI/Swagger UI, Foundry server-rendered UI, super-admin capability management UI, job admin API, task polling + SSE.

**Key dependencies:** all above + `jobs`, `axum-extra` 0.10 (cookies, multipart), `tower` 0.5, `tower-http` 0.6 (cors, trace, compression, ServeDir), `jsonwebtoken` 9, `blake3` 1, `hmac` 0.12, `askama` 0.12, `utoipa` 5, `utoipa-swagger-ui` 9, `time` 0.3.

#### `src/main.rs`

`tokio::main`. Initialises telemetry (JSON logs + optional OTLP). Builds `AppState::from_env()`. Verifies LLM providers. Registers `TranscribeVideoCapability` (needs `Arc<JobExecutor>` from state). Starts `JobSchedulerService` (cron loop). Resolves `assets_dir`. Assembles full router with layered middleware. Binds `CONUSAI_SERVER__HOST:PORT`.

#### `src/state.rs`

`AppState { registry, rate_limiter, llm, file_store: Option<Arc<dyn ObjectStore>>, presigned_tokens, thread_store, audit_store, workspace_store, workspace_content, conversation_service, tool_admin, job_registry, job_executor, job_admin, pool: Option<PgPool>, embedding_service: Arc<dyn EmbeddingService>, vector_store: Arc<PgVectorStore>, realtime_service: Option<Arc<RealtimeService>> }`.

`AppState::from_env()`: `PgPool` → `LlmRegistry` → `ToolRegistry` (with discovery) → MinIO file store → `PostgresThreadStore` → `PostgresAuditStore` → `EmbeddingService` (`EMBEDDING_BACKEND`: `"local"` → `LocalEmbeddingService`, `"openai"` → `OpenAiEmbeddingService`, default → `NoopEmbeddingService`) → `PgVectorStore` → `PostgresWorkspaceStore` → `MinioWorkspaceContent` or `NoopWorkspaceContent` → `DefaultConversationService` → `JobContext/JobRegistry/JobExecutor/JobAdmin` → `RealtimeService`.

`AppState::with_in_memory_stores()`: `pool=None`, `file_store=None`, `NoopEmbeddingService`, `PgVectorStore::noop()`, `realtime_service=None`.

`build_job_registry(ctx)` — pre-registers `CapabilityHealthCheckJob`, `AuditLogCleanupJob` (scheduled) and `VideoTranscriptionJob` (background).

#### `src/capabilities/transcribe_video.rs`

`TranscribeVideoCapability { manifest, executor: Arc<JobExecutor> }` implements `CapabilityProvider`. Tool `transcribe(file_id)` enqueues a `VideoTranscriptionJob` and returns `{ task_id, status: "queued", poll_url }` instantly. Registered at startup (not from TOML).

#### Middleware (`src/mw/`)

| File | Middleware | Purpose |
|---|---|---|
| `mw/api_key.rs` | `extract_api_key` | Reads `X-API-Key`; hashes with BLAKE3; validates against `API_KEYS` env (`<blake3_hex>:<tenant_id>:<plan>` CSV). Sets `ResolvedTenant` if valid; falls through to JWT if absent; rejects 401 if present but invalid. |
| `mw/tenant.rs` | `extract_tenant` | Skips if `ResolvedTenant` already set. Production (`JWT_SECRET` set): HS256 Bearer JWT or session cookie. Dev: `X-Tenant-ID` header or `dev` default + Enterprise plan. Inserts `ResolvedTenant(TenantContext)` extension. |
| `mw/plan.rs` | `enforce_plan` | Validates `ResolvedTenant` has a recognised `PlanTier`. Runs after `api_key` + `tenant`. |
| `mw/admin.rs` | `require_super_admin_jwt` / `require_super_admin_session` | Enforces `role = SuperAdmin` from JWT extension or session cookie. Applied to `/admin/*` and `/super-admin/*` routes. |
| `mw/rate_limit.rs` | `RateLimiter` | Per-tenant 60-second sliding window. `check(tenant_id, limit_rpm) -> bool`. Plan-based limits: Free 10 / Pro 60 / Enterprise 600 RPM. |
| `mw/request_id.rs` | `inject_request_id` | Reads `X-Request-ID` or generates UUID. Echoes in response header. For JSON 4xx/5xx bodies, injects `request_id` into `{"error": {...}}` (reads + rewrites body up to 1 MiB). |
| `mw/trace.rs` | `propagate_trace` | Extracts W3C `traceparent`/`tracestate` via `TraceContextPropagator`; sets as parent span. |

**Middleware stack order** (outer → inner):
`TraceLayer` → `inject_request_id` → `propagate_trace` → `extract_api_key` → `extract_tenant` → `enforce_plan`

#### Routes (`src/routes/`)

**Router assembly:**
- `public_router()` — `/health`, `POST /v1/auth/login`, `GET /docs`, `GET /openapi.json`
- `protected_router()` — all `/v1/*`, `/mcp`, and `/api/realtime/*` routes behind full middleware stack
- `admin_router()` — `/admin/*` routes behind `require_super_admin_jwt`
- `ui_router()` — `/`, `/login`, `/logout`, `/ui/*`, `/super-admin/*`
- `GET /metrics` — Prometheus text exposition (no auth)
- `nest_service("/assets", ServeDir)` — static assets

| File | Endpoint(s) | Purpose |
|---|---|---|
| `routes/health.rs` | `GET /health` | Returns `{status, version (CARGO_PKG_VERSION), capabilities}`. Utoipa-documented. |
| `routes/auth.rs` | `POST /v1/auth/login` | `{email, password, tenant_id?}` → HS256 JWT (`{access_token, token_type, expires_in: 86400, tenant_id}`). Dev: issues JWT for any non-empty email. Production: validates `DEV_PASSWORD`. Claims: `sub`, `tenant_id`, `plan`, `role`, `exp`. |
| `routes/chat.rs` | `POST /v1/chat/completions` | OpenAI-compatible chat. Blocking: returns `ChatResponse`. Streaming: SSE with OpenAI delta chunks. Rate-limited; `max_tokens` clamped by plan. |
| `routes/agent.rs` | `POST /v1/agent/completions` | Thread-aware tool-calling loop. Blocking + streaming (`"stream": true`) modes. Thread resolution: explicit `thread_id` wins; else workspace node `metadata.thread_id` (lazy `bind_thread`). History load + summary injection + `ContextBuilder` preamble (6000 chars). Anthropic `tool_use` rounds (≤ `max_turns`, capped by plan). Streaming: SSE OpenAI chunks + `tool_call_start` / `tool_call_result` events. After every turn: `WorkspaceStore::index_content` (last 30 msgs). `gen_ai.*` span attributes. Returns `thread_id` in response. |
| `routes/capabilities.rs` | `GET /v1/capabilities` | Lists all enabled capabilities (name, version, description, kind, tags, tools). |
| `routes/search.rs` | `GET /v1/capabilities/search?q=&limit=` | Semantic search via Postgres pgvector ANN. On each request, capability cards are upserted into `capability_embeddings` (hash-based change detection). Query is embedded and top-N retrieved via cosine ANN. Falls back to local substring match on failure. `limit` default 5, max 20. Returns `{source: "vector"}` on fast path. |
| `routes/mcp.rs` | `POST /mcp` | JSON-RPC 2.0. Methods: `initialize`, `tools/list`, `tools/call` (`capability__tool` slug split). |
| `routes/files.rs` | `POST /v1/files`, `GET /v1/files/{token}` | Multipart upload to MinIO at `tenants/{tenant_id}/{uuid}/{filename}`; returns 1-h TTL download token. Bearer-JWT-protected token-gated streaming download. |
| `routes/audit.rs` | `GET /v1/audit?limit=` | Lists `AuditEvent`s newest-first. Default 50, max 500. Returns `{events, count}`. |
| `routes/workspaces.rs` | workspace routes | `create`, `tree`, `search`, `get_node`, `delete_node`, `get_content`, `patch_content`, `move_node` (POST), `share_node` (POST), `unshare_node` (POST `/v1/workspaces/{id}/unshare`). |
| `routes/threads.rs` | `GET /v1/threads/{id}/messages` | Returns paginated message list for a thread. |
| `routes/realtime.rs` | `GET /api/realtime/workspace` | WebSocket upgrade; broadcasts `WorkspaceChangeEvent` to the caller via `RealtimeService`. |
| `routes/admin_capabilities.rs` | 11 admin routes | `list`, `get_one`, `get_manifest`, `create`, `update`, `set_enabled`, `delete_one`, `reload_one`, `reload_all`, `validate`, `test_invoke`. All require `super_admin` JWT role. |
| `routes/admin_jobs.rs` | 4 admin routes | `list_jobs` (`GET /admin/jobs`), `get_job` (`GET /admin/jobs/{name}`), `run_now` (`POST /admin/jobs/{name}/run`), `list_tasks` (`GET /admin/tasks`). All require `super_admin` JWT role. |

**OpenAPI** — `ApiDoc` with `#[derive(OpenApi)]`. Security schemes: `bearer_auth` (HS256 JWT), `api_key_auth` (X-API-Key header), `cookie_auth` (conusai_session cookie). Swagger UI at `/docs`; spec JSON at `/openapi.json`.

#### UI Routes (`src/ui/`)

| File | Endpoint(s) | Purpose |
|---|---|---|
| `ui/routes.rs` | — | `ui_router()` — assembles all UI routes. Super-admin sub-router guarded by `require_super_admin_session`. |
| `ui/handlers/auth.rs` | `GET /login`, `POST /login`, `GET /logout` | Login form: name + plan + (super-admin password for elevated role). Signs `SessionUser` cookie via `ui/session.rs`. |
| `ui/session.rs` | — | `SessionUser { name, plan, role, exp }`. HMAC-SHA256 signed, base64url-encoded as `payload.sig`. `UI_SESSION_KEY` env (default dev secret). `TTL_SECS = 86400`. Axum `FromRequestParts` extractor auto-redirects to `/login` on missing/invalid/expired cookie. `SessionUser::tenant_context()` → `TenantContext(tenant_id = CONUSAI_UI_TENANT_ID ∥ "dev")`. |
| `ui/handlers/app.rs` | `GET /` | Renders `app.html` (Askama) with recent threads, capabilities, workspace tree, user info. |
| `ui/handlers/chat.rs` | `POST /ui/stream` | SSE stream — `{message, thread_id?, model?, workspace_node_id?}` → in-process `agent::stream_agent`. |
| `ui/handlers/upload.rs` | `POST /ui/upload` | Multipart → MinIO. Returns `{id, filename, size, download_url}`. |
| `ui/handlers/invoice.rs` | `POST /ui/extract-invoice` | Token → MinIO bytes → `InvoicePipeline::extract_from_bytes` → `InvoiceData` JSON. No agent loop. |
| `ui/handlers/super_admin.rs` | `/super-admin/*` | Askama-rendered capability management: list, new form, create, detail, update, toggle, delete, reload. Delegates to `RegisteredToolAdmin`. |

**Templates** (`templates/`): `app.html`, `login.html`, `partials/composer.html`, `shared/head.html`, `super_admin/{layout,list,new,detail}.html`.

**Assets** (`assets/`):
- `css/style.css` — design system + workspace styles (~1320 lines; editorial paper-canvas aesthetic)
- `js/app.js` — streaming + composer + workspace-select handler (~660 lines)
- `js/workspace.js` — tree + search + dialogs + context menu (~750 lines)
- `icons/icons.svg` — SVG sprite
- `images/` — favicon, logo light/dark

**CORS** — `build_cors()`: `WEB_ORIGIN` env (comma-separated, default `http://localhost:3000`). Allowed methods: GET, POST, PATCH, DELETE, OPTIONS. Exposed headers: `X-Request-ID`.

---

### 4.4 `evals` — Evaluation Framework

| Path | Purpose |
|---|---|
| `src/main.rs` | `clap` CLI: `run --suite <name> --dataset <path?> --model <id>` and `list`. |
| `src/runners/invoice.rs` | Loads JSONL `EvalSample { image_path, expected }`; runs `InvoicePipeline`; scores with `InvoiceScorer`. |
| `src/runners/ocr_quality.rs` | Sends image through `ocr-service` capability via gateway; scores against expected text snippets. Requires `GATEWAY_URL`. |
| `src/scorers/mod.rs` | `ScorerResult { score, passed, details }`. `InvoiceScorer { pass_threshold = 0.8 }` — case-insensitive string match + `abs(diff) < 0.01` for numbers; compares 7 invoice fields. |
| `src/report.rs` | Summary table: totals, pass count, average, ALL PASS / SOME FAILED. |
| `datasets/invoice.jsonl` | Invoice extraction test samples. |
| `datasets/ocr_quality.jsonl` | OCR quality samples. |

---

## 5. `capabilities/` — Zero-Code Extension

Drop a folder with a `capability.toml` (and optionally a `.wasm`) into `capabilities/`; the registry auto-discovers and loads it at startup or on admin reload.

### Capability kinds

| Kind | Runtime | Wire format |
|---|---|---|
| `mcp` | External HTTP/stdio process | JSON-RPC 2.0 |
| `wasm` | Wasmtime (`wasm32-wasip1`) | Exported WASM functions |
| `chain` (hardcoded) | In-process Rig pipeline | `InvoicePipeline` / `ContractPipeline` / `OcrProvider` |
| `chain` (data-driven) | `LlmChainTool` via `LlmRegistry` | TOML `[chain]` block + `PromptTemplate` |
| `docker` | Container (reserved) | TBD |
| `native` | In-process Rust | `BuiltinProvider` (fs, cargo) |

### Data-driven chain capabilities

Any `capability.toml` with `kind = "chain"` and a `[chain]` section gets a `LlmChainTool` provider automatically — **no Rust code required**:

```toml
kind = "chain"
[chain]
model = "opus"                   # LlmRegistry alias or concrete model id
system_prompt = "You are …"
prompt_template = "{{input.text}}"
vision = false
max_tokens = 2048
output_schema = { ... }          # optional JSON Schema for response validation
```

`{{input.*}}` and `{{tenant.id}}` / `{{tenant.plan}}` placeholders resolved via `PromptTemplate`.

### Discovered capabilities

| Folder | Kind | Tools | Notes |
|---|---|---|---|
| `file-storage/` | mcp | `upload_file`, `download_file`, `presigned_url` | Manifest only; actual storage in `routes/files.rs`. |
| `google-workspace/` | mcp | `list_files`, `read_document`, `append_to_sheet`, `send_email` | OAuth2: drive.readonly, documents.readonly, spreadsheets, gmail.send. |
| `invoice-processing/` | chain | `extract_invoice`, `validate_invoice` | `InvoicePipeline`; default model `claude-opus-4-7`; max 20 MB; png/jpeg/jpg/pdf. |
| `contract-processing/` | chain | `extract_contract`, `summarise_contract` | `ContractPipeline`. |
| `ocr-service/` | chain | `extract_text` | `OcrProvider`; default model `claude-sonnet-4-6`. |
| `runtime-echo/` | chain | echo | Minimal chain capability for runtime testing. |
| `template-wasm/` | wasm | `ping` | Loads `capability.wasm`; exports `ping() -> i32 = 42`. |

### Capability selection: `invoice-processing` vs `ocr-service`

| Need | Correct capability |
|---|---|
| Invoice, bill, purchase order → structured fields | `invoice-processing__extract_invoice` |
| Contract, letter, generic document → raw text | `ocr-service__extract_text` |

Rich `description` fields in `capability.toml` are loaded verbatim into Anthropic tool definitions at startup — Claude selects the correct tool deterministically via semantic matching without any code classifier.

---

## 6. Configuration & Environment

| Var | Default | Purpose |
|---|---|---|
| `CONUSAI_SERVER__HOST` | `0.0.0.0` | Bind address |
| `CONUSAI_SERVER__PORT` | `8080` | Listen port |
| `CONUSAI_CAPABILITIES_DIR` | `./capabilities` | Capability discovery root |
| `CONUSAI_WORKSPACE_ROOT` | `/tmp/conusai/workspaces` | Tenant workspace root (native tools) |
| `CONUSAI_UI_ASSETS` | (auto-detected) | Override UI assets directory |
| `CONUSAI_UI_TENANT_ID` | `dev` | Tenant ID used by the UI session |
| `CONUSAI_TEST_MODE` | — | `1` → all stores in-memory; no Postgres/MinIO |
| `CONUSAI_MAX_CAPABILITIES` | `64` | Admin limit: max registered capabilities |
| `CONUSAI_MAX_MANIFEST_BYTES` | `65536` | Admin limit: max manifest size |
| `CONUSAI_MAX_WASM_BYTES` | `8388608` | Admin limit: max WASM binary size (8 MiB) |
| `DATABASE_URL` | — | Postgres connection string (e.g. `postgres://conusai:conusai@localhost:5432/conusai`) |
| `EMBEDDING_BACKEND` | — | `openai` → `OpenAiEmbeddingService`; `local` → `LocalEmbeddingService` (fastembed); default → `NoopEmbeddingService` |
| `MINIO_ENDPOINT` / `S3_ENDPOINT` | — | MinIO/S3 endpoint (enables file + workspace content stores) |
| `MINIO_BUCKET` | `conusai` | Storage bucket |
| `MINIO_ACCESS_KEY` / `MINIO_SECRET_KEY` | `minioadmin` | Dev credentials |
| `ANTHROPIC_API_KEY` | — | Required for all LLM calls |
| `JWT_SECRET` | — | HS256 key; if unset → dev mode (`X-Tenant-ID`) |
| `API_KEYS` | — | `<blake3_hex>:<tenant_id>:<plan>` CSV for API key auth |
| `DEV_PASSWORD` | — | Password for `POST /v1/auth/login` in production mode |
| `UI_SESSION_KEY` | (dev secret) | HMAC key for UI session cookies |
| `WEB_ORIGIN` | `http://localhost:3000` | Allowed CORS origins (comma-separated) |
| `OTLP_ENDPOINT` | — | OTel collector gRPC endpoint (e.g. `http://localhost:4317`) |
| `RUST_LOG` | — | `tracing` filter string |
| `CONUSAI_LLM__DEFAULT` | `opus` | Global default LLM alias |
| `CONUSAI_LLM__ALIASES__OPUS__MODEL` | `claude-opus-4-7` | Override opus model id |
| `CONUSAI_LLM__ALIASES__HAIKU__MODEL` | `claude-haiku-4-5-20251001` | Override haiku model id |

---

## 7. Startup & Request Lifecycle

### Gateway startup

1. `tokio::main` → `common::telemetry::init("agent-gateway", "info")` — JSON logs + optional OTLP.
2. `AppState::from_env()`:
   - `CONUSAI_TEST_MODE=1` → `with_in_memory_stores()` (no Docker needed).
   - Otherwise: `PgPool` → `build_llm_registry()` → `LlmRegistry`; `ToolRegistry::with_default_factories(llm)` pre-seeds four factories; `ToolDiscovery::from_env().discover_into(&mut registry)` loads capabilities; MinIO client via `AmazonS3Builder`; `PostgresThreadStore`, `PostgresAuditStore`, `EmbeddingService`, `PgVectorStore`, `PostgresWorkspaceStore`, `MinioWorkspaceContent`, `RealtimeService`.
3. `verify_llm_providers` — validates all LLM aliases at startup (warn-only).
4. Router assembled: public + metrics + protected + admin + ui + assets.
5. Layers applied (outermost first): CORS → `TraceLayer` → `inject_request_id` → `propagate_trace` → (per-router) `extract_api_key` → `extract_tenant` → `enforce_plan`.
6. `axum::serve` on `{HOST}:{PORT}`.

### Request lifecycle

```
HTTP request
  └─► axum router
        ├─ public_router  ──► /health, /v1/files/{token}, /v1/auth/login, /docs
        ├─ GET /metrics   ──► Prometheus text (no auth)
        └─ protected_router (inject_request_id → propagate_trace →
                             extract_api_key → extract_tenant → enforce_plan)
              ├─ /v1/chat/completions        → chat.rs    (Rig agent.prompt; SSE or blocking)
              ├─ /v1/agent/completions       → agent.rs   (tool loop, thread, workspace)
              │     ├─ ConversationService::resolve_for_node  (lazy bind_thread)
              │     ├─ ContextBuilder::build_for_node(6000)  (system preamble)
              │     ├─ ThreadStore::messages (history injection)
              │     └─ Anthropic tool_use rounds (≤ plan.max_turns)
              │           ├─ ToolExecutor::invoke(registry, cap, tool, input, tenant)
              │           │     ├─ chain  → InvoiceProvider / ContractProvider / OcrProvider / LlmChainTool
              │           │     ├─ wasm   → WasmProvider (wasmtime)
              │           │     ├─ mcp    → McpProvider (JSON-RPC 2.0)
              │           │     └─ native → BuiltinProvider (fs, cargo)
              │           └─ on end_turn:
              │                 ├─ ConversationService::append_message
              │                 └─ WorkspaceStore::index_content (last 30 msgs)
              ├─ /v1/capabilities            → capability list
              ├─ /v1/capabilities/search     → Postgres pgvector ANN + fallback
              ├─ /mcp                        → JSON-RPC 2.0 dispatcher
              ├─ /v1/files                   → MinIO multipart upload
              ├─ /v1/audit                   → AuditStore::list
              └─ /v1/workspaces              → WorkspaceStore + WorkspaceContentStore
        ├─ admin_router (require_super_admin_jwt)
        │     ├─ /admin/capabilities/*       → RegisteredToolAdmin CRUD
        │     └─ /admin/jobs/*, /admin/tasks  → JobAdmin
        └─ ui_router
              ├─ /                          → Foundry app shell (Askama)
              ├─ /login, /logout
              ├─ /ui/stream                 → SSE agent stream (in-process)
              ├─ /ui/upload                 → MinIO upload
              ├─ /ui/extract-invoice        → InvoicePipeline direct
              └─ /super-admin/*             → capability management UI (require_super_admin_session)
```

---

## 8. HTTP API Surface

### Public

| Method | Path | Purpose |
|---|---|---|
| GET | `/health` | Status / version / capability count |
| POST | `/v1/auth/login` | Exchange credentials for HS256 JWT |
| GET | `/docs` | Swagger UI |
| GET | `/openapi.json` | OpenAPI 3.1 spec |
| GET | `/metrics` | Prometheus text format |

### Protected (Bearer JWT or `X-API-Key`)

| Method | Path | Purpose |
|---|---|---|
| POST | `/v1/chat/completions` | OpenAI-compatible chat (SSE optional) |
| POST | `/v1/agent/completions` | Tool-calling agent loop (blocking + SSE) |
| GET | `/v1/capabilities` | List enabled capabilities |
| GET | `/v1/capabilities/search?q=&limit=` | Semantic capability search (Postgres pgvector ANN + fallback) |
| POST | `/mcp` | MCP JSON-RPC 2.0 |
| POST | `/v1/files` | Multipart upload (MinIO) |
| GET | `/v1/files/{token}` | Token-gated streaming download (1 h TTL) |
| GET | `/v1/audit?limit=` | Audit log (newest-first; default 50, max 500) |
| POST | `/v1/workspaces` | Create folder or conversation |
| GET | `/v1/workspaces/tree?parent_id=` | Immediate children visible to caller |
| GET | `/v1/workspaces/search?q=&limit=` | Text search + substring fallback |
| GET | `/v1/workspaces/{id}` | Single node (NotFound if not accessible) |
| GET | `/v1/workspaces/{id}/content` | Read markdown body |
| PATCH | `/v1/workspaces/{id}/content` | Save body → index_content |
| POST | `/v1/workspaces/{id}/move` | Reparent node |
| POST | `/v1/workspaces/{id}/share` | Owner-only: add user to `shared_with` |
| POST | `/v1/workspaces/{id}/unshare` | Owner-only: remove user from `shared_with` |
| DELETE | `/v1/workspaces/{id}` | Recursive delete + MinIO cleanup |
| GET | `/v1/tasks` | List background task statuses |
| GET | `/v1/tasks/{id}` | Get single task status |
| GET | `/v1/tasks/{id}/sse` | SSE stream for task lifecycle events |
| GET | `/v1/threads/{id}/messages` | Paginated message list for a thread |
| GET | `/api/realtime/workspace` | WebSocket — workspace change event stream |

### Super-admin (JWT with `role = super_admin`)

| Method | Path | Purpose |
|---|---|---|
| GET | `/admin/capabilities` | List all capabilities (enabled + disabled) |
| POST | `/admin/capabilities` | Create capability (validate + persist + register) |
| POST | `/admin/capabilities/reload` | Hot-reload all capability directories |
| POST | `/admin/capabilities/validate` | Validate manifest TOML without persisting |
| POST | `/admin/capabilities/test` | Test-invoke a capability tool |
| GET | `/admin/capabilities/{name}` | Get capability summary |
| GET | `/admin/capabilities/{name}/manifest` | Get raw TOML |
| PATCH | `/admin/capabilities/{name}` | Update manifest |
| PATCH | `/admin/capabilities/{name}/enabled` | Toggle enabled (`{enabled: bool}`) |
| DELETE | `/admin/capabilities/{name}` | Delete capability + filesystem cleanup |
| POST | `/admin/capabilities/{name}/reload` | Hot-reload single capability |
| GET | `/admin/jobs` | List all registered jobs |
| GET | `/admin/jobs/{name}` | Get single job summary |
| POST | `/admin/jobs/{name}/run` | Enqueue a background job immediately |
| GET | `/admin/tasks` | List all task statuses (admin view) |

---

## 9. Security

- **Authentication:** HS256 JWT (`JWT_SECRET`) in production; API key (BLAKE3-hashed, `API_KEYS` env) as first-class auth method; HMAC-SHA256 session cookies for UI; dev fallback `X-Tenant-ID`.
- **Authorization:** `UserRole { User, Admin, SuperAdmin }` in JWT claims + session cookie. Super-admin middleware enforces role on `/admin/*` and `/super-admin/*`.
- **Path safety:** `safe_join` rejects `..` in all tenant FS access.
- **Storage isolation:** MinIO keys under `tenants/{tenant_id}/`; Postgres rows filtered by `tenant_id`; pgvector embeddings share tables but are namespaced by `tenant_id`.
- **Workspace ACL:** private-by-default; per-node `shared_with`; non-owners receive `NotFound` (no existence leakage).
- **API key security:** only BLAKE3 hash stored in env var; raw key never persisted.
- **WASM sandboxing:** Wasmtime engine; `MAX_WASM_SIZE_BYTES = 10 MB`; only allowlisted exports invoked.
- **CORS:** configurable `WEB_ORIGIN`; `allow_credentials: true`.
- **Request correlation:** `X-Request-ID` echoed in response + injected into JSON error bodies.

---

## 10. Observability

- **Structured logs:** JSON via `tracing-subscriber` (env-filter from `RUST_LOG`).
- **Distributed tracing:** W3C `traceparent`/`tracestate` propagation; OTLP export to otel-collector → Jaeger.
- **Metrics — OTel (OTLP + Prometheus):** Single `SdkMeterProvider` with both readers.
  - `conusai.agent` meter: `agent.tool.invocations`, `agent.tool.errors`, `agent.tool.duration_ms`, `agent.llm.requests`, `agent.llm.input_tokens`, `agent.llm.output_tokens`.
  - `conusai.storage` meter: `storage.request.duration_ms`, `storage.request.errors`.
- **Span attributes:** `tenant_id`, `plan`, `tool.cap`, `tool.name`, `error.type`, `gen_ai.system`, `gen_ai.request.model`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `thread_id`.
- **Prometheus endpoint:** `GET /metrics` (text/plain 0.0.4).
- **Healthcheck:** `GET /health` → `{status, version, capabilities}`.

---

## 11. Build & Deploy

### Local development

```bash
# Infrastructure only (Postgres + MinIO)
./start.sh infra

# Full stack (infra + build + run gateway)
./start.sh full

# With observability (Jaeger + OTLP)
./start.sh observability
```

### Cargo builds

```bash
cargo build --release --bin agent-gateway
cargo build --release --bin evals
cargo build --release --target wasm32-wasip1 -p capability-example
```

### Docker

```bash
docker build -t conusai-gateway:0.1.0 .
docker compose --profile full up -d
```

### Build profiles

| Profile | `opt-level` | `lto` | `codegen-units` | `strip` |
|---|---|---|---|---|
| `release` | 3 | `thin` | 1 | `symbols` |
| `dev` | 0 | off | default | off |

---

## 12. Tests & Quality

- **common (22 tests):** path traversal rejection, safe joins, MCP serde, `ApiError` fields, limit invariants, thread/message/tool-call serde roundtrips, `WorkspaceNode` serde, every `validate_name` branch, `join_virtual_path`, `effective_user_id`.
- **agent-core (8 tests):** registry register/get/tag-search; manifest embedding text; nonexistent-dir handling; WASM `ping` execution; `PostgresThreadStore` pool construction; `PgVectorStore::noop()` returns empty results.
- **Total:** 30+ lib tests (`cargo test --workspace`). Integration tests in `crates/agent-core/tests/` and `crates/agent-gateway/tests/` require live Postgres + MinIO.
- **Quality gates:** `cargo clippy --workspace -- -D warnings`, `cargo fmt --all`.
- **Test mode:** `CONUSAI_TEST_MODE=1` replaces all Postgres + MinIO stores with in-memory equivalents — no Docker required.

---

## 13. Design Patterns

- **LLM abstraction layer:** `LlmProvider` trait + `LlmRegistry` with 4-step resolution. Adding a new LLM provider requires one new file in `llm/providers/` — zero changes to routes, chains, or agent loop. `verify_llm_providers` validates registry at startup.
- **Data-driven chain tools:** `LlmChainConfig` in TOML + `PromptTemplate` + `LlmChainTool` — new LLM-backed tools require only a `capability.toml` with `[chain]` section, no Rust code.
- **ToolProvider + ToolProviderFactory:** `ToolProvider` (`manifest`, `invoke`, `invoke_typed`, `tool_definitions`) + `ToolProviderFactory` (`supports`, `create`). Four factories pre-registered: `McpFactory`, `WasmFactory`, `ChainFactory(llm)`, `BuiltinFactory`. New capability kind: one new provider + one factory, zero registry changes.
- **Super-admin capability CRUD:** `RegisteredToolAdmin` (in-process) + `/admin/capabilities/*` (API) + `/super-admin/*` (UI) — create, update, toggle, reload, test-invoke, validate without restart. `FilesystemStore` provides atomic manifest writes (`.tmp` → rename).
- **Typed ID newtypes:** `ThreadId`, `NodeId`, `TenantId`, `UserId`, `ToolId` — compile-time safety; `serde(transparent)` wire format.
- **ConversationService:** single source of truth for thread lifecycle. Coordinates `ThreadStore` + `WorkspaceStore::bind_thread`.
- **Multitenant isolation:** JWT/API-key auth; tenant-prefixed paths/keys/collections; plan-based token limits + rate limits + turn caps; `UserRole` RBAC; `safe_join` path safety.
- **Persistent memory:** `PostgresThreadStore` (sqlx, `threads`/`messages` tables); auto-summarisation via Haiku background task.
- **Workspace hierarchy:** folders + conversations as `.md` in MinIO; Postgres `workspace_nodes` + `content_embeddings` tables; private-by-default ACL; per-node thread binding; ContextBuilder ancestor context injection.
- **Observability by default:** structured JSON logs, OTel spans with W3C propagation, `#[instrument]` on every significant async method.
- **Scheduled + background jobs (v0.3):** `ScheduledJob` trait (cron, `tokio-cron-scheduler`) + `BackgroundJob` trait (on-demand, `JobExecutor` in-memory tracker). `JobRegistry` wires both kinds with shared `JobContext`. `JobSchedulerService` spawns cron loop at startup. SSE polling at `GET /v1/tasks/{id}/sse`. In-memory only; Apalis/Postgres migration-ready (trait unchanged).

---

## 14. Status

- **Version:** 0.3.1
- **State:** operational, 100% verified end-to-end (per [verify.md](verify/verify.md)).

**Implemented:** multitenancy (JWT + API key + session), `UserRole` (User/Admin/SuperAdmin), `CompletionProvider` + `LlmRegistry` abstraction layer, `AnthropicProvider` via `rig-core` 0.36 + `rig-postgres` 0.2.5, data-driven `PromptChainCapability` + `PromptTemplate`, `ConversationService` trait + `DefaultConversationService`, super-admin capability CRUD API + Foundry UI, invoice + contract + OCR pipelines, YAML/TOML capability discovery, `ToolKind::Chain` + four factories, OpenAI-compatible chat, SSE streaming, tool-calling agent loop (blocking + streaming), MCP JSON-RPC, **Postgres pgvector semantic capability search**, MinIO file storage, WASM execution (wasmtime 44), Google Workspace manifest, evals framework (invoice + OCR), Jaeger/OTLP tracing, per-tenant rate limiting, **Postgres-backed thread/workspace/audit stores**, `gen_ai.*` OTel span attributes, W3C traceparent propagation, native filesystem + cargo tools, cargo-chef Docker image, hierarchical workspace + `content_embeddings`, append-only audit log, Prometheus metrics, OpenAPI + Swagger UI, request-ID correlation, typed ID newtypes, CORS, **scheduled jobs (`ScheduledJob` + `tokio-cron-scheduler`)**, **background tasks (`BackgroundJob` + `JobExecutor` + SSE polling)**, **`TranscribeVideoCapability`** (enqueues `VideoTranscriptionJob` → Whisper API), **`GET /v1/tasks`, `GET /v1/tasks/{id}/sse`, `GET /v1/threads/{id}/messages`, `GET /api/realtime/workspace`**, **`/admin/jobs/*` REST API**, **workspace indexer (`WorkspaceIndexer`, `EmbeddingService`, `RealFsWatcher`)**, **realtime WebSocket service (`RealtimeService`)**, **`runtime-echo` capability**.

**Reserved / future:** `Docker` capability kind, external MCP server federation, multi-instance deployment, audit retention/compaction, billing/quota enforcement, OIDC integration, multi-layer context budgeting, live document mode, agent-callable workspace toolkit, additional LLM providers (OpenAI, Ollama, Bedrock), Apalis/Postgres job persistence, whisper-rs local transcription.

---

## 15. File-Tree Summary

```
conusai-platform/
├── docker-compose.yml
├── Makefile
├── start.sh / stop.sh
├── docs/
│   ├── arch.md                          # this document
│   ├── ui-design.md
│   ├── verify/verify.md
│   └── adr/005-workspace-access-control.md
│
└── apps/backend/
    ├── Cargo.toml                       # workspace (resolver = "3"; rust-version = "1.88")
    ├── Dockerfile                       # 4-stage cargo-chef
    ├── rust-toolchain.toml              # stable + wasm32-wasip1
    │
    ├── crates/
    │   ├── common/
    │   │   └── src/
    │   │       ├── lib.rs, types.rs, error.rs, config/mod.rs, telemetry.rs
    │   │       ├── http_client.rs, mcp.rs, wasm.rs, db.rs, limits.rs, path_safety.rs
    │   │       ├── eval.rs, audit.rs, metrics.rs
    │   │       └── memory/{mod,thread,workspace,store,inmem}.rs
    │   │
    │   ├── agent-core/
    │   │   └── src/
    │   │       ├── lib.rs
    │   │       ├── llm/{mod,types,error,provider,registry,streaming,providers/anthropic}.rs
    │   │       ├── agent/{mod,builder,hooks,runtime}.rs
    │   │       ├── context/{mod,tenant,conversation}.rs
    │   │       ├── prompt/{mod,template}.rs
    │   │       ├── chains/{mod,extraction,invoice,contract,llm_chain}.rs
    │   │       ├── memory/{mod,postgres_thread_store,postgres_workspace_store,
    │   │       │            postgres_audit_store,minio_workspace_content,context_builder,
    │   │       │            truncator}.rs
    │   │       ├── indexing/{mod,coco_indexer,embedding_service,local_embedding_service,
    │   │       │             real_fs_watcher}.rs
    │   │       ├── realtime/{mod,...}.rs
    │   │       ├── vector_store/{mod,postgres}.rs
    │   │       └── tools/{mod,manifest,card,provider,registry,discovery,store,validator,
    │   │                  admin,embedding,executor,mcp_adapter,wasm_loader,
    │   │                  providers/{mod,chain,mcp,wasm,builtin},
    │   │                  builtin/{mod,fs,cargo,card}}.rs
    │   │
    │   └── agent-gateway/
    │       ├── src/
    │       │   ├── main.rs, state.rs
    │       │   ├── mw/{mod,api_key,tenant,plan,admin,rate_limit,request_id,trace}.rs
    │       │   ├── routes/{mod,health,auth,chat,agent,capabilities,search,mcp,files,
    │       │   │           audit,workspaces,threads,realtime,admin_capabilities,admin_jobs}.rs
    │       │   └── ui/{mod,routes,session,view,
    │       │           handlers/{mod,auth,app,chat,upload,invoice,super_admin}}.rs
    │       ├── assets/
    │       │   ├── css/style.css         (~1320 lines, design system)
    │       │   ├── js/app.js             (~660 lines, streaming + composer)
    │       │   ├── js/workspace.js       (~750 lines, tree + search + dialogs)
    │       │   ├── icons/icons.svg
    │       │   └── images/{favicon,logo-light,logo-dark}.png
    │       └── templates/
    │           ├── app.html, login.html
    │           ├── partials/composer.html
    │           ├── shared/head.html
    │           └── super_admin/{layout,list,new,detail}.html
    │
    ├── evals/
    │   ├── src/{main,config,report,
    │   │        runners/{mod,invoice,ocr_quality},
    │   │        scorers/mod}.rs
    │   └── datasets/{invoice,ocr_quality}.jsonl
    │
    └── capabilities/
        ├── file-storage/        capability.toml (mcp)
        ├── google-workspace/    capability.toml (mcp)
        ├── contract-processing/ capability.toml (chain)
        ├── invoice-processing/  capability.toml (chain)
        ├── ocr-service/         capability.toml (chain)
        ├── runtime-echo/        capability.toml (chain)
        └── template-wasm/       capability.toml + .wasm (wasm)
```
