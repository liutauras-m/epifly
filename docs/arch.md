# ConusAI Platform — Architecture & Functionality

A production-grade multitenant AI agent platform. The monorepo contains a **Rust + Rig** backend (`apps/backend/`) and WASM/MCP capabilities. The built-in **Foundry UI** (served by the gateway at `GET /`) provides workspace management, agent chat, file upload, and invoice extraction without a separate frontend app.

---

## 3. Infrastructure

### [docker-compose.yml](../docker-compose.yml) (v3.9)

| Service | Image | Ports | Profiles | Purpose |
|---|---|---|---|---|
| `qdrant` | `qdrant/qdrant:v1.13.6` | 6333 (REST), 6334 (gRPC) | infra, full | Vector DB for semantic capability search |
| `minio` | `quay.io/minio/minio:RELEASE.2025-04-22T22-12-26Z` | 9000 (S3), 9001 (Console) | infra, full | S3-compatible object storage |
| `minio-init` | (same) | — | full | Initializes bucket `conusai` |
| `agent-gateway` | (built locally) | 8080 | full | HTTP API gateway |
| `jaeger` | `jaegertracing/all-in-one:1.58` | 16686 (UI), 14317 (OTLP) | observability, full | Trace UI |
| `otel-collector` | `otel/opentelemetry-collector-contrib:0.123.0` | 4317 (gRPC), 4318 (HTTP) | observability, full | OTLP receiver/exporter |

- **Profiles:** `infra` (core), `full` (everything), `observability` (tracing).
- **Volumes:** `qdrant_data:/qdrant/storage`, `minio_data:/data`, `./capabilities:/app/capabilities:ro`, `./scripts/otel-collector.yaml:/etc/otelcol-contrib/config.yaml:ro`.
- **MinIO dev creds:** `minioadmin` / `minioadmin`.
- All services declare healthchecks; `agent-gateway` waits for `qdrant` (healthy) and `minio-init` (completed).

### [start.sh](../start.sh)

1. Accepts profile (`infra` | `full` | `observability`).
2. Loads `.env` (or copies from `.env.example`).
3. Brings up infra: `docker compose --profile $PROFILE up -d --wait`.
4. Polls Qdrant until healthy.
5. If `full`: builds `agent-gateway` (`cargo build --release --bin agent-gateway`).
6. Counts capabilities under [capabilities/](../capabilities) and prints summary URLs (gateway 8080, Qdrant 6333, MinIO 9001).

### [rust-toolchain.toml](../rust-toolchain.toml)

```toml
channel = "stable"
targets = ["wasm32-wasip1"]
components = ["rustfmt", "clippy", "rust-src"]
```

### Documentation files

| File | Purpose |
|---|---|
| [docs/plan.md](plan.md) | Rig.rs alignment refactor plan v0.2.0 (Steps 1–6 complete). |
| [docs/tenant.md](tenant.md) | Multitenancy design — `TenantContext`, `TenantClaims`, `extract_tenant`, dev-mode/JWT mode, isolation surfaces. |
| [docs/about.md](about.md) | Platform overview — purpose, key capabilities, target use cases. |
| [docs/verify/verify.md](verify/verify.md) | End-to-end Docker verification plan, JWT helpers, curl recipes (Phases 0–14 incl. workspace, audit, UI, ToolProvider regression). |
| [docs/ui-design.md](ui-design.md) | Design tokens (colour, type, spacing, motion) and component recipes. |
| [docs/adr/005-workspace-access-control.md](adr/005-workspace-access-control.md) | ADR for the private-by-default + selective-sharing ACL model. |

---

## 3. Crates

### 3.1 [`crates/common`](../crates/common) — Shared Utilities

**Purpose:** foundational types, errors, telemetry, MCP JSON-RPC 2.0, WASM loader, config, path safety.

| File | Purpose |
|---|---|
| [src/lib.rs](../crates/common/src/lib.rs) | Re-exports modules; defines `prelude` (`Result`, `ConusAiError`). |
| [src/error.rs](../crates/common/src/error.rs) | `ConusAiError` enum (Config / Tool / Wasm / WasmRuntime / Mcp / Rig / Qdrant / Storage / Validation / NotFound / Api / Io / Other). `WasmRuntime`, `Rig`, `Qdrant` are string-wrapped variants (rig's error types are not `'static + Send` across all 0.9.x releases; wasmtime's `Error` is in `agent-core`). `ApiError { code, message }`. |
| [src/config/mod.rs](../crates/common/src/config/mod.rs) | `AppConfig`, `ServerConfig`, `QdrantConfig`, `TelemetryConfig`. Layered loading via `figment` (TOML + env + YAML). |
| [src/telemetry.rs](../crates/common/src/telemetry.rs) | `TelemetryGuard` (RAII). `init(name, level)` — JSON `tracing-subscriber` + optional OTLP trace + metrics export. When `OTLP_ENDPOINT` is set, builds a single `SdkMeterProvider` with both a Prometheus reader and an OTLP `PeriodicReader` — `opentelemetry_prometheus::exporter().build()` is called exactly once (a second call on the same `Registry` would panic with "Duplicate metrics collector registration attempted"). |
| [src/http_client.rs](../crates/common/src/http_client.rs) | `build_client()` → `reqwest::Client` (60 s timeout, UA `conusai-platform/0.1`). |
| [src/mcp.rs](../crates/common/src/mcp.rs) | `JsonRpcRequest` / `JsonRpcResponse` / `JsonRpcError` (jsonrpc 2.0). |
| [src/wasm.rs](../crates/common/src/wasm.rs) | `WasmLoader` wrapping `wasmtime::Engine`: `load_bytes`, `load_file`, `new_store`. |
| [src/limits.rs](../crates/common/src/limits.rs) | Constants: `MAX_PROMPT_TOKENS=128k`, `MAX_RESPONSE_TOKENS=16k`, `MAX_CAPABILITY_SIZE_BYTES=50 MB`, `MAX_WASM_SIZE_BYTES=10 MB`, `REQUEST_TIMEOUT_SECS=120`, `MAX_CONCURRENT_AGENTS=64`, `MAX_MESSAGES_PER_THREAD=10_000`, `MAX_MESSAGES_BEFORE_SUMMARY=50`. |
| [src/path_safety.rs](../crates/common/src/path_safety.rs) | `safe_join()` (rejects `..`), `join_under_tenant(root, tenant_id, rel)`. |
| [src/eval.rs](../crates/common/src/eval.rs) | Trait stubs shared by the evals crate. |
| [src/memory/thread.rs](../crates/common/src/memory/thread.rs) | `Thread { id (ULID), tenant_id, title, created_at, last_active, message_count, summary, metadata }`. `Message { role, content, tool_calls, timestamp, seq }`. `ToolCall { id, name, input, output }`. |
| [src/memory/workspace.rs](../crates/common/src/memory/workspace.rs) | `NodeKind { Folder, Conversation, File }`, `WorkspaceNode { id, tenant_id, owner_id, parent_id, kind, name, virtual_path, last_modified, shared_with, metadata }`. Helpers: `new_folder`, `new_conversation`, `validate_name` (rejects empty / >255 / `/` / `\` / `..` / leading `.` and enforces `.md` for conversations), `join_virtual_path`, `effective_user_id` (maps `None` → `"__dev__"`). |
| [src/memory/store.rs](../crates/common/src/memory/store.rs) | Three async traits — `ThreadStore` (`create`, `get`, `messages`, `append`, `list`, `set_summary`, `set_title`); `WorkspaceStore` (`create_folder`, `create_conversation`, `list_accessible_children`, `get_accessible_node`, `get_ancestors`, `move_node`, `delete_node`, `share_node`, `unshare_node`, `bump_last_modified`, `search_nodes`, `index_content`, `bind_thread`); `WorkspaceContentStore` (`read`, `write`, `delete`). All methods take `tenant_id: &str` and (where access matters) a `user_id: &str`, avoiding any circular dep with agent-core. |
| [src/memory/inmem.rs](../crates/common/src/memory/inmem.rs) | In-memory implementations of all four store traits for test/CI use. `InMemoryThreadStore` (`Mutex<HashMap<(tenant,id), Thread>>` + messages map), `InMemoryWorkspaceStore` (`Mutex<HashMap<Ulid, WorkspaceNode>>` + `content_text` map; full ACL, recursive delete, substring search), `InMemoryWorkspaceContent` (`Mutex<HashMap<(tenant,path), String>>`; `read` returns `""` on miss), `InMemoryAuditStore` (`Mutex<Vec<AuditEvent>>`; newest-first sort). Zero external dependencies — no Qdrant or MinIO required. Activated via `CONUSAI_TEST_MODE=1`. |
| [src/audit.rs](../crates/common/src/audit.rs) | `AuditEvent { id (ULID), tenant_id, timestamp, action, tool?, status, duration_ms?, metadata }` with builder helpers (`with_tool`, `with_status`, `with_duration_ms`, `with_metadata`). `AuditStore` async trait: `append`, `list(tenant, limit)`. |
| [src/metrics.rs](../crates/common/src/metrics.rs) | OpenTelemetry meter helpers (`qdrant_duration_ms`, `qdrant_errors`, `llm_requests`, `llm_input_tokens`, `llm_output_tokens`) plus `kv(key, value)` for label construction. |

**Tests:** path-traversal rejection, valid joins, MCP serialization, `ApiError` fields, limit invariants, thread/message/tool-call serde roundtrips.

---

### 3.2 [`crates/agent-core`](../crates/agent-core) — Agent Runtime & Tool Registry

**Purpose:** Rig integration; tool discovery / registration; tool execution (MCP, WASM, pipeline, builtin) via `ToolProvider` trait; tenant context; invoice/contract pipelines.

#### Tools subsystem ([src/tools/](../crates/agent-core/src/tools))

| File | Purpose |
|---|---|
| [mod.rs](../crates/agent-core/src/tools/mod.rs) | Re-exports `card`, `discovery`, `embedding`, `executor`, `manifest`, `mcp_adapter`, `provider`, `registry`, `wasm_loader`, `providers/*`, `builtin/*`. |
| [provider.rs](../crates/agent-core/src/tools/provider.rs) | `ToolProvider` async trait — `manifest()`, `invoke(tool, input, tenant) -> Value`, `tool_definitions()` (default impl), `invoke_typed<I,O>` (default impl, `where Self: Sized` — serializes typed input, calls `invoke`, deserializes result). Free function `invoke_typed_dyn` for `&dyn ToolProvider` callers. `ToolProviderFactory` trait — `supports(kind, name) -> bool`, `create(card) -> Arc<dyn ToolProvider>` — pluggable factory registered in `ToolRegistry`. |
| [manifest.rs](../crates/agent-core/src/tools/manifest.rs) | `ToolManifest { name, version, description, kind, tools, config, tags }`; `ToolKind { Mcp, Wasm, Chain, Docker, Native }` (`#[serde(rename_all = "lowercase")]` — wire value is `chain`); `ToolDef { name, description, input_schema }`. `from_yaml`, `from_file`, `embedding_text`. |
| [card.rs](../crates/agent-core/src/tools/card.rs) | `ToolCard { id (UUID), manifest, source_path, embedding_id }`. |
| [registry.rs](../crates/agent-core/src/tools/registry.rs) | `ToolRegistry` — `cards: HashMap<String, ToolCard>` (metadata) + `providers: HashMap<String, Arc<dyn ToolProvider>>` (executors) + `factories: Vec<Box<dyn ToolProviderFactory>>` (pluggable factories). `with_default_factories()` pre-registers `McpFactory`, `WasmFactory`, `ChainFactory`, `BuiltinFactory`. `with_builtin()` = `with_default_factories()` + pre-register the builtin tool card. `register_factory()`, `register_provider`, `register_card`, `get_provider`, `get`, `search_by_tag`, `all`, `len`, `is_empty`, `load_from_dir(dir)` — consults factory list first, falls back to legacy `provider_for()` match, then card-only. |
| [discovery.rs](../crates/agent-core/src/tools/discovery.rs) | `ToolDiscovery` — `from_env()` reads `CONUSAI_CAPABILITIES_DIR` (default `./capabilities`); `discover()` creates a new `ToolRegistry` and calls `discover_into`; `discover_into(&mut registry)` loads capabilities into a pre-seeded registry (preserving existing factories). Use `ToolRegistry::with_default_factories()` before `discover_into` to ensure all YAML-loaded capabilities find a factory. |
| [embedding.rs](../crates/agent-core/src/tools/embedding.rs) | `ToolEmbedding::describe(card)` — returns text used for semantic search (delegates to `manifest.embedding_text()`). |
| [executor.rs](../crates/agent-core/src/tools/executor.rs) | `ToolExecutor::invoke(registry, cap_name, tool, input, tenant)` — dispatches via `registry.get_provider(cap_name)?.invoke(...)`. `#[instrument]` span carries `tool.cap`, `tool.name`, `tenant_id`. Metrics: `tool_invocations`, `tool_duration_ms`, `tool_errors` (incremented on every error path). `tool_definitions_from_manifest` helper for Anthropic format. |
| [mcp_adapter.rs](../crates/agent-core/src/tools/mcp_adapter.rs) | `McpAdapter` — JSON-RPC 2.0 HTTP client (`call`, `list_tools`, `call_tool`) for external MCP servers. |
| [wasm_loader.rs](../crates/agent-core/src/tools/wasm_loader.rs) | `WasmToolLoader` (wraps `wasmtime::Engine`). `load(card)` reads `card.source_path/capability.wasm`; `invoke_i32`, `invoke_tool(card, tool, input)`. |
| [providers/mod.rs](../crates/agent-core/src/tools/providers/mod.rs) | Re-exports `chain`, `mcp`, `wasm`, `builtin` sub-modules. Legacy `provider_for(card)` match retained as fallback inside `load_from_dir`; superseded by the factory list for all new code. |
| [providers/chain.rs](../crates/agent-core/src/tools/providers/chain.rs) | `InvoiceProvider`, `ContractProvider`, `OcrProvider` — each implements `ToolProvider`; wraps the corresponding `*Pipeline` struct. `ChainFactory` implements `ToolProviderFactory` (`supports` matches `ToolKind::Chain`; `create` routes by `manifest.name`). All error paths emit `tracing::error!` with `tenant_id`, `tool`, and `error` fields. |
| [providers/mcp.rs](../crates/agent-core/src/tools/providers/mcp.rs) | `McpProvider` wrapping `McpAdapter`. `McpFactory` implements `ToolProviderFactory`. |
| [providers/wasm.rs](../crates/agent-core/src/tools/providers/wasm.rs) | `WasmProvider` wrapping `WasmToolLoader`. `WasmFactory` implements `ToolProviderFactory`. |
| [providers/builtin.rs](../crates/agent-core/src/tools/providers/builtin.rs) | `BuiltinProvider` — hard-coded manifest for `native-tools` (`read_file`, `write_file`, `run_cargo`). `BuiltinFactory` implements `ToolProviderFactory` (`supports` matches `ToolKind::Native`). |
| [builtin/fs.rs](../crates/agent-core/src/tools/builtin/fs.rs) | `read_file` / `write_file` — tenant-scoped filesystem access via `safe_join`. |
| [builtin/cargo.rs](../crates/agent-core/src/tools/builtin/cargo.rs) | `run_cargo` — allowlisted subcommands via `tokio::process::Command`. |
| [builtin/card.rs](../crates/agent-core/src/tools/builtin/card.rs) | `builtin_tool_card()` — builds a `ToolCard` with `kind: Native`. |

#### Agent subsystem ([src/agent/](../crates/agent-core/src/agent))

| File | Purpose |
|---|---|
| [builder.rs](../crates/agent-core/src/agent/builder.rs) | `GeneralAgentBuilder` (fluent — `model`, `preamble`, `max_tokens`, `with_tenant`, `build`); `build_for_tenant`. Honors plan-based `max_tokens`. `GeneralAgent::prompt(text)` wraps the Rig Anthropic agent. |
| [runtime.rs](../crates/agent-core/src/agent/runtime.rs) | `AgentRuntime` = `GeneralAgent` + `ToolRegistry`; `new`, `for_tenant`, `run`, `registry`. |

#### Context subsystem ([src/context/](../crates/agent-core/src/context))

| File | Purpose |
|---|---|
| [tenant.rs](../crates/agent-core/src/context/tenant.rs) | `PlanTier { Free, Pro, Enterprise }` with `max_tokens()` (4k/16k/128k) and `rate_limit_rpm()` (10/60/600). `TenantContext { tenant_id, user_id, plan, workspace_root }` with `tenant_root()`, `safe_path(rel)`, `storage_prefix()` (`tenants/{id}/`), `qdrant_collection(kind)` (`{kind}_{tenant_id}`), `span_fields()`. `TenantClaims { sub, tenant_id, plan, exp }` for JWT. |
| [mod.rs](../crates/agent-core/src/context/mod.rs) | Also exposes `ConversationContext` for chat history. |

#### Chains ([src/chains/](../crates/agent-core/src/chains))

| File | Purpose |
|---|---|
| [extraction.rs](../crates/agent-core/src/chains/extraction.rs) | `ExtractionPipeline` async trait — `model_id()`, `system_prompt()`, `run(bytes: Vec<u8>, tenant: Option<&TenantContext>) -> Result<Output>` (primary entry point, mirrors `rig::pipeline::Op::call` signature). Default impls: `extract_from_bytes` (delegates to `run`), `extract_as_value` (delegates to `extract_from_bytes`, serializes to `serde_json::Value`). `where Self: Sized` not required — `run` is dyn-compatible. |
| [invoice.rs](../crates/agent-core/src/chains/invoice.rs) | `InvoiceLineItem`, `InvoiceData` (~20 fields, `JsonSchema`). `InvoicePipeline::new()` (default `claude-opus-4-7`), `with_model`, `with_tenant`. Inherent methods: `extract_from_image_path`, `extract_from_bytes` (public, called from CLI + UI handler). Private `run_extraction(&[u8])` holds core vision logic — base64-encodes bytes, sends to Claude vision with strict JSON schema prompt, strips markdown fences, parses to `InvoiceData`. Implements `ExtractionPipeline` (delegates `run()` to `run_extraction`). |
| [contract.rs](../crates/agent-core/src/chains/contract.rs) | `ContractParty`, `ContractData`. `ContractPipeline` — same structure as `InvoicePipeline`. `extract_from_document_path`, `extract_from_bytes`, private `run_extraction`. Implements `ExtractionPipeline`. |

#### Memory subsystem ([src/memory/](../crates/agent-core/src/memory))

| File | Purpose |
|---|---|
| [qdrant_helpers.rs](../crates/agent-core/src/memory/qdrant_helpers.rs) | Shared Qdrant REST helpers used by all three stores. `point_id(key) -> u64` (SHA-256 → first 8 bytes), `zero_vec() -> Vec<f32>` (4-dim placeholder). `QdrantClient` struct with `ensure_collection(col, keyword_fields, text_fields)`, `upsert_point`, `scroll_filter`, `patch_payload`, `delete_point`, `get_point` — all with OTel duration + error metrics. Eliminates ~150 lines of boilerplate that were duplicated across the three Qdrant stores. |
| [qdrant_store.rs](../crates/agent-core/src/memory/qdrant_store.rs) | `QdrantThreadStore` — implements `ThreadStore` using Qdrant REST as a document store (not vector search). Uses 4-dim zero vectors; SHA-256 → u64 point IDs; collection per tenant (`threads_{tenant_id}`); payload indices on `type`, `thread_id`, `tenant_id`. `scroll_filter()` for all queries. Background `tokio::spawn` for auto-summarisation when `message_count % MAX_MESSAGES_BEFORE_SUMMARY == 0`, calling Claude Haiku via Anthropic API. All 7 trait methods instrumented with `#[instrument]` (OTel spans). |
| [qdrant_workspace_store.rs](../crates/agent-core/src/memory/qdrant_workspace_store.rs) | `QdrantWorkspaceStore` — implements `WorkspaceStore`. Mirrors the thread store pattern (REST, 4-dim zero vectors, SHA-256→u64 point IDs) but as a hierarchical metadata store. Per-tenant collection `workspaces_{tenant_id}`. Payload schema: `id`, `tenant_id`, `owner_id`, `parent_id` (string, `""` for root), `kind` (`folder`/`conversation`/`file`), `name`, `virtual_path`, `last_modified` (RFC 3339), `shared_with: [user_id]`, `metadata: object`, `content_text` (truncated body for full-text search). On collection creation: keyword indexes on `tenant_id`/`owner_id`/`parent_id`/`kind`/`shared_with` and **text** indexes on `name`/`content_text` (`tokenizer: word, lowercase: true, min_token_len: 2, max_token_len: 128`). `node_to_point` seeds `content_text: ""` so new nodes are searchable by name immediately. `patch_payload(...)` is a targeted Qdrant payload SET via `POST /collections/{col}/points/payload` — used for `move_node`, `share_node`, `unshare_node`, `bind_thread`, `bump_last_modified`, and `index_content`, so `content_text` and other untouched keys are preserved across metadata updates. Access filter `tenant_id == X AND (owner_id == U OR shared_with ∋ U)` is built by `access_filter()` using the **struct form** of `min_should` (`{conditions, min_count}`) — Qdrant rejects the integer shorthand. `get_accessible_node` returns `NotFound` (never `Forbidden`) for non-owners to avoid leaking existence. `delete_node` walks children via worklist (avoids deep async recursion). `search_nodes` issues per-token `text_match` over `name` ∪ `content_text`, falling back to a substring scan when the index is missing or returns nothing. `index_content` truncates to 32 KB at a UTF-8 boundary and SETs `content_text` + `last_modified` in one targeted call. `ensure_text_indexes` lazily backfills text indexes on collections that pre-date this feature. |
| [minio_workspace_content.rs](../crates/agent-core/src/memory/minio_workspace_content.rs) | `MinioWorkspaceContent` — implements `WorkspaceContentStore` against an `Arc<dyn ObjectStore>` (the same MinIO client wired into `AppState.file_store`). Object keys are built as `tenants/{tenant_id}/workspaces/{virtual_path}` via `OsPath::from(...)`. `read` returns `""` on `NotFound` so newly-created conversations work without a write-first; `write` puts UTF-8 bytes; `delete` is best-effort (silently OK on missing). |
| [context_builder.rs](../crates/agent-core/src/memory/context_builder.rs) | `ContextBuilder` — assembles a workspace-scoped system preamble. `build_for_node(tenant, node_id, max_chars)` resolves the effective `user_id` (via `effective_user_id`), fetches `get_ancestors(...)` (already access-filtered), tries `{ancestor_path}/CONTEXT.md` then `{ancestor_path}/README.md` from MinIO for each ancestor folder, then loads the selected node body if it is a `Conversation`. Sections are joined with `\n\n---\n\n`, prefixed with the `virtual_path` as an H2, and **truncated from the front** (oldest ancestor first) until total length ≤ `max_chars`. Output begins with `# Workspace context\n` so the agent recognises it. Never errors hard — on any access failure returns an empty string. Used by `routes/agent.rs::build_ctx` with `max_chars = 6000` whenever `workspace_node_id` is present. |
| [qdrant_audit.rs](../crates/agent-core/src/memory/qdrant_audit.rs) | `QdrantAuditStore` — implements `common::audit::AuditStore`. Per-tenant collection `audit_{tenant_id}`, 4-dim zero vectors, SHA-256→u64 point ID derived from `AuditEvent.id` (a ULID). `append` ensures the collection then upserts a single point with the full event as payload. `list(tenant, limit)` scrolls with `order_by: { key: "timestamp", direction: "desc" }` and deserialises payloads back into `AuditEvent`. Retention is currently unbounded — no expiry, no compaction. |

#### Native tools subsystem ([src/tools/builtin/](../crates/agent-core/src/tools/builtin))

| File | Purpose |
|---|---|
| [builtin/fs.rs](../crates/agent-core/src/tools/builtin/fs.rs) | `read_file(workspace_root, input)` / `write_file(workspace_root, input)` — tenant-scoped filesystem access via `safe_join` (rejects `..`). Uses `tokio::fs`. |
| [builtin/cargo.rs](../crates/agent-core/src/tools/builtin/cargo.rs) | `run_cargo(workspace_root, input)` — runs `cargo {check,test,build,clippy,fmt}` via `tokio::process::Command`; returns stdout/stderr/exit_code as JSON. Allowlisted subcommands only. |
| [builtin/card.rs](../crates/agent-core/src/tools/builtin/card.rs) | `builtin_tool_card()` — builds a `ToolCard` with `kind: Native` exposing `read_file`, `write_file`, `run_cargo` with full JSON schemas. Auto-registered at gateway startup. |

**Public re-exports** (via [`lib.rs`](../crates/agent-core/src/lib.rs)): `GeneralAgent`, `GeneralAgentBuilder`, `ToolDiscovery`, `ToolRegistry`, `ToolProviderFactory`, `PlanTier`, `TenantClaims`, `TenantContext`, `ContextBuilder`, `MinioWorkspaceContent`, `QdrantAuditStore`, `QdrantThreadStore`, `QdrantWorkspaceStore`, `ContractData`, `ContractParty`, `ContractPipeline`, `ExtractionPipeline`, `InvoiceData`, `InvoiceLineItem`, `InvoicePipeline`, `builtin_tool_card`.

---

### 3.3 [`crates/agent-gateway`](../crates/agent-gateway) — HTTP API

**Purpose:** OpenAI-compatible chat/agent endpoints, tool calling, MCP dispatch, capability search, file upload/download, JWT auth, rate limiting.

| File | Purpose |
|---|---|
| [src/main.rs](../crates/agent-gateway/src/main.rs) | Tokio entrypoint. Initializes telemetry, builds `AppState`, mounts public + protected routers, applies `CorsLayer` + `TraceLayer` + tenant + trace middleware, binds `0.0.0.0:8080`. |
| [src/state.rs](../crates/agent-gateway/src/state.rs) | `AppState { registry: Mutex<ToolRegistry>, rate_limiter, file_store: Option<Arc<dyn ObjectStore>>, qdrant_url, presigned_tokens: Mutex<HashMap>, thread_store: Arc<dyn ThreadStore>, audit_store: Arc<dyn AuditStore>, workspace_store: Arc<dyn WorkspaceStore>, workspace_content: Arc<dyn WorkspaceContentStore> }`. `from_env()` first checks `CONUSAI_TEST_MODE=1` and delegates to `with_in_memory_stores()` when set; otherwise calls `ToolRegistry::with_default_factories()` + `ToolDiscovery::from_env().discover_into(&mut registry)` (factories registered before YAML load so every discovered capability finds a factory), initializes MinIO if `MINIO_ENDPOINT`/`S3_ENDPOINT` is set, instantiates `QdrantThreadStore`, `QdrantAuditStore`, `QdrantWorkspaceStore`, and either `MinioWorkspaceContent` or `NoopWorkspaceContent`. `with_in_memory_stores()` uses the same factory-first pattern; wires the four `InMemory*` stores — no Qdrant or MinIO required; all data lost on process exit. |

#### Middleware ([src/mw/](../crates/agent-gateway/src/mw))

| File | Purpose |
|---|---|
| [tenant.rs](../crates/agent-gateway/src/mw/tenant.rs) | `extract_tenant`: production (when `JWT_SECRET` is set) requires HS256 `Authorization: Bearer …` and decodes `TenantClaims`; dev mode (no secret) accepts `X-Tenant-ID` header or defaults to `dev`/Enterprise. Inserts `ResolvedTenant(TenantContext)` extension. |
| [trace.rs](../crates/agent-gateway/src/mw/trace.rs) | `propagate_trace`: extracts W3C `traceparent`/`tracestate` via `TraceContextPropagator` and sets parent on the current span. |
| [rate_limit.rs](../crates/agent-gateway/src/mw/rate_limit.rs) | `RateLimiter` per-tenant 60 s sliding window; `check(tenant_id, limit_rpm) -> bool`. |

#### Routes ([src/routes/](../crates/agent-gateway/src/routes))

| File | Endpoint(s) | Purpose |
|---|---|---|
| [health.rs](../crates/agent-gateway/src/routes/health.rs) | `GET /health` | Status, version, capability count. |
| [chat.rs](../crates/agent-gateway/src/routes/chat.rs) | `POST /v1/chat/completions` | OpenAI-compatible chat (streaming via SSE or blocking). Builds Rig Anthropic agent with system preamble + `max_tokens`, enforces per-tenant rate limits. |
| [agent.rs](../crates/agent-gateway/src/routes/agent.rs) | `POST /v1/agent/completions` | Thread-aware tool-calling agent loop with blocking and streaming (`"stream": true`) modes. Accepts optional `thread_id` and `workspace_node_id`. Thread resolution rule: explicit `thread_id` wins; else if `workspace_node_id` is set, `WorkspaceStore::get_accessible_node(...)` reads `metadata.thread_id` and either reuses the bound thread or lazily creates one and writes the binding via `WorkspaceStore::bind_thread`. Loads history + injects thread summary as system context. When `workspace_node_id` is set, also runs `ContextBuilder::build_for_node(..., 6000)` and concatenates the result into the system preamble. Persists user message before the loop and assistant reply after; auto-sets thread title from first reply. After every completed turn (both paths) reads the last 30 messages and re-indexes them via `WorkspaceStore::index_content` so chat history is searchable. Up to 5 tool-use rounds. Streaming path emits OpenAI SSE chunks + `tool_call_start` / `tool_call_result` events so clients can follow tool execution in real-time. Accumulates `gen_ai.*` span attributes (model, input/output tokens). Returns `thread_id` in response. |
| [threads.rs](../crates/agent-gateway/src/routes/threads.rs) | Thread CRUD | `create_thread`, `list_threads`, `get_thread`, `get_messages`, `append_message`. Delegates to `AppState::thread_store` (`QdrantThreadStore`). |
| [capabilities.rs](../crates/agent-gateway/src/routes/capabilities.rs) | `GET /v1/capabilities` | Lists capabilities (name, version, description, kind, tags, tools) with tenant + plan. |
| [search.rs](../crates/agent-gateway/src/routes/search.rs) | `GET /v1/capabilities/search?q=…&limit=…` | Semantic search via Qdrant (64-dim deterministic hash embeddings). On first call per tenant, creates collection `capabilities_{tenant_id}` and upserts capability vectors. Falls back to local substring match if Qdrant is unreachable. |
| [mcp.rs](../crates/agent-gateway/src/routes/mcp.rs) | `POST /mcp` | JSON-RPC 2.0 dispatcher. Methods: `initialize` (server info), `tools/list` (all tool defs), `tools/call` (`capability__tool`, splits name, looks up provider via `registry.get_provider()`, dispatches via `provider.invoke()`). |
| [files.rs](../crates/agent-gateway/src/routes/files.rs) | `POST /v1/files`, `GET /v1/files/{token}` | Multipart upload to MinIO under `tenants/{tenant_id}/{uuid}/{filename}`; returns 1-h TTL download token. Download endpoint is public (token-gated) and streams back the object. |
| [audit.rs](../crates/agent-gateway/src/routes/audit.rs) | `GET /v1/audit?limit=` | Lists recent `AuditEvent`s for the calling tenant from `audit_{tenant_id}`, ordered by `timestamp desc`. `limit` defaults to 50, capped at 500. Returns `{events, count}`. |
| [workspaces.rs](../crates/agent-gateway/src/routes/workspaces.rs) | `POST /v1/workspaces`, `GET /v1/workspaces/tree`, `GET /v1/workspaces/search`, `GET /v1/workspaces/{id}`, `GET/PATCH /v1/workspaces/{id}/content`, `POST /v1/workspaces/{id}/move`, `POST /v1/workspaces/{id}/share`, `POST /v1/workspaces/{id}/unshare`, `DELETE /v1/workspaces/{id}` | Hierarchical workspace CRUD over `WorkspaceStore` + `WorkspaceContentStore`. `create` validates the name eagerly (returns 400) and, for conversations, writes an empty `.md` to MinIO after Qdrant upsert. `patch_content` writes MinIO first, then `index_content` updates `content_text`. `tree` lists immediate children via `list_accessible_children`. `search` runs token-based text_match across `name` ∪ `content_text` (limit defaults to 40, capped at 200). `delete` is recursive in Qdrant; for conversation leaves it best-effort deletes the MinIO object first. All mutating routes rate-limit; `Validation`/`NotFound`/other → `400`/`404`/`500`. |
| [mod.rs](../crates/agent-gateway/src/routes/mod.rs) | `public_router()` (health + file download), `protected_router()` (chat, agent, capabilities, capability search, MCP, files upload, 5 thread routes, `/v1/audit`, 9 workspace routes). |

#### UI Routes ([src/ui/](../crates/agent-gateway/src/ui))

| File | Endpoint(s) | Purpose |
|---|---|---|
| [routes.rs](../crates/agent-gateway/src/ui/routes.rs) | — | `ui_router()` — assembles all UI routes. |
| [handlers/auth.rs](../crates/agent-gateway/src/ui/handlers/auth.rs) | `GET /login`, `POST /login`, `GET /logout` | HMAC-signed session cookie (`conusai_session`) — see [`ui/session.rs`](../crates/agent-gateway/src/ui/session.rs). Login form: name + plan tier. Cookie HMAC key from `UI_SESSION_KEY` (defaults to a dev-only secret). |
| [view.rs](../crates/agent-gateway/src/ui/view.rs) | — | Askama template view structs (`LoginView`, `AppView`, `RecentView`, `CapView`) — pure data containers; no routing logic. |
| [handlers/app.rs](../crates/agent-gateway/src/ui/handlers/app.rs) | `GET /` | Renders `app.html` with greeting, recents, capabilities, user info. |
| [handlers/chat.rs](../crates/agent-gateway/src/ui/handlers/chat.rs) | `POST /ui/stream` | SSE stream — accepts `{message, thread_id?, model?, workspace_node_id?}`, builds a `ChatRequest` with `stream: true`, calls `agent::stream_agent` in-process. The `workspace_node_id` carries through to `routes/agent.rs` so the chat is bound to the active node, gets workspace context injection, and is re-indexed for search. |
| [handlers/upload.rs](../crates/agent-gateway/src/ui/handlers/upload.rs) | `POST /ui/upload` | Multipart → MinIO. Returns `{id, filename, size, download_url}`. |
| [handlers/invoice.rs](../crates/agent-gateway/src/ui/handlers/invoice.rs) | `POST /ui/extract-invoice` | Direct pipeline: token → MinIO bytes → `InvoicePipeline::extract_from_bytes` → `InvoiceData` JSON. No agent loop. |

**Session bridge** ([`ui/session.rs`](../crates/agent-gateway/src/ui/session.rs)): `SessionUser { name, plan, exp }` is signed with HMAC-SHA256, base64url-encoded as `payload.sig`, and verified in constant time. `SessionUser::tenant_context()` produces a shared `TenantContext { tenant_id = "dev" (or `CONUSAI_UI_TENANT_ID`), user_id = Some(name), plan, workspace_root }` so `/v1/*` and `/ui/*` resolve to the same tenant + ACL space in dev mode. The `SessionUser` extractor auto-redirects to `/login` on missing/invalid/expired cookie. In production (`JWT_SECRET` set) the protected `/v1/*` routes ignore session cookies and require a Bearer JWT — see [`docs/tenant.md`](tenant.md).

---

### 3.4 [`examples/invoice-cli`](../examples/invoice-cli) — Standalone CLI

[`main.rs`](../examples/invoice-cli/src/main.rs):

- `clap` `Args { image, --model (default claude-opus-4-7), --tenant-id (default conusai-demo), --plan (default enterprise), --json }`.
- Builds a `TenantContext` and `InvoicePipeline::with_model().with_tenant()`.
- Runs `extract_from_image_path()` and prints either raw JSON or a colored, sectioned report (header, invoice details, issuer, billed-to, line items, totals, notes).

---

### 3.5 [`evals`](../evals) — Evaluation Framework

| Path | Purpose |
|---|---|
| [src/main.rs](../evals/src/main.rs) | `clap` CLI with `run --suite … --dataset … --model …` and `list`. |
| [src/runners/mod.rs](../evals/src/runners/mod.rs) | `run_suite(suite, dataset, model)` dispatcher. |
| [src/runners/invoice.rs](../evals/src/runners/invoice.rs) | Loads JSONL `EvalSample { image_path, expected }`; runs `InvoicePipeline`; scores with `InvoiceScorer`; prints report. |
| [src/runners/threads.rs](../evals/src/runners/threads.rs) | Multi-turn thread recall eval: creates thread, runs conversation turns via gateway, asks a recall question, scores keyword presence. Requires `GATEWAY_URL` env. |
| [src/runners/ocr_quality.rs](../evals/src/runners/ocr_quality.rs) | OCR quality eval: sends image through `ocr-service` capability via gateway, scores against expected text snippets. |
| [src/scorers/mod.rs](../evals/src/scorers/mod.rs) | `ScorerResult { score, passed, details }`. `InvoiceScorer { pass_threshold = 0.8 }` — case-insensitive string match + `abs(diff) < 0.01` for numbers; compares `invoice_number`, `invoice_date`, `issuer_name`, `billed_to_name`, `currency`, `total_amount`, `status`. |
| [src/report.rs](../evals/src/report.rs) | Prints a summary table (totals, pass count, average, ALL PASS / SOME FAILED). |
| [src/config.rs](../evals/src/config.rs) | `EvalConfig { suite, model, dataset_path }`. |
| [datasets/invoice.jsonl](../evals/datasets/invoice.jsonl) | Invoice extraction test samples. |
| [datasets/threads.jsonl](../evals/datasets/threads.jsonl) | Thread recall test samples (`turns`, `recall_question`, `expected_keywords`). |
| [datasets/ocr_quality.jsonl](../evals/datasets/ocr_quality.jsonl) | OCR quality test samples (`image_path`, `expected_snippets`). |

---

## 4. [`capabilities/`](../capabilities) — Zero-Code Extension

Drop a folder with a `capability.toml` (and optionally an implementation) into `capabilities/`; the registry auto-discovers it on startup.

### Capability kinds (`ToolKind`)

| Kind | Runtime | Implementation | Tool format |
|---|---|---|---|
| `mcp` | External process | JSON-RPC 2.0 over HTTP / stdio | MCP standard |
| `wasm` | Wasmtime | `wasm32-wasip1` module | Exported WASM functions |
| `chain` | In-process Rig | Claude vision + structured extraction (`ExtractionPipeline`) | Rig agent + tool defs |
| `docker` | Container | (reserved / future) | TBD |
| `native` | In-process Rust | `crate::tools` (fs, cargo) | Built-in — no YAML manifest |

### Discovered capabilities

| Folder | Kind | Tools | Notes |
|---|---|---|---|
| [file-storage](../capabilities/file-storage/capability.toml) | mcp | `upload_file`, `download_file`, `presigned_url` | Manifest only — actual storage handled directly by [`routes/files.rs`](../crates/agent-gateway/src/routes/files.rs) using `object_store`. |
| [google-workspace](../capabilities/google-workspace/capability.toml) | mcp | `list_files`, `read_document`, `append_to_sheet`, `send_email` | OAuth2 scopes: `drive.readonly`, `documents.readonly`, `spreadsheets`, `gmail.send`. |
| [invoice-processing](../capabilities/invoice-processing/capability.toml) | chain | `extract_invoice`, `validate_invoice` | Backed by [`InvoicePipeline`](../crates/agent-core/src/chains/invoice.rs) via `InvoiceProvider`; default model `claude-opus-4-7`, max image 20 MB, formats `png/jpeg/jpg/pdf`. |
| [contract-processing](../capabilities/contract-processing/capability.toml) | chain | `extract_contract`, `summarise_contract` | Backed by [`ContractPipeline`](../crates/agent-core/src/chains/contract.rs) via `ContractProvider`. |
| [ocr-service](../capabilities/ocr-service/capability.toml) | chain | `extract_text` | Reuses `InvoicePipeline` for vision OCR via `OcrProvider`; default model `claude-sonnet-4-6`. |
| [template-wasm](../capabilities/template-wasm/capability.toml) | wasm | `ping` | Loads `capability.wasm` exporting `ping() -> i32 = 42`. |


### Capability selection: `invoice-processing` vs `ocr-service`

These two capabilities are intentionally **non-overlapping** — the LLM (Claude) selects the right one via tool description quality and Qdrant semantic embeddings:

| Need | Correct capability |
|---|---|
| Invoice, bill, purchase order, accounts-payable document → **structured fields** | `invoice-processing__extract_invoice` |
| Contract, letter, handwritten note, generic document → **raw text** | `ocr-service__extract_text` |

`invoice-processing__extract_invoice` handles the vision step internally (Claude vision + strict JSON schema in one call). Calling `ocr-service` before it is redundant and adds unnecessary latency. The rich `description` fields in both `capability.toml` files — loaded verbatim into tool definitions at startup — make this routing deterministic without any code-level classifier.

---

## 5. Other Top-Level Folders

### `wasm/` (not yet created)
Reserved for WASM capability source crates targeting `wasm32-wasip1`. Drop a crate here and build with `cargo build --target wasm32-wasip1`.

### [`scripts/`](../scripts)

| File | Purpose |
|---|---|
| [docker-verify.sh](../scripts/docker-verify.sh) | Automated end-to-end Docker verification (per [verify.md](../verify.md)). |
| [otel-collector.yaml](../scripts/otel-collector.yaml) | OTel Collector config — OTLP gRPC/HTTP receivers, Jaeger exporter. |

---

## 6. Runtime Flow

### Startup (gateway)

1. `tokio::main` → `common::telemetry::init("agent-gateway", "info")` (JSON logs + optional OTLP).
2. `AppState::from_env()` → `ToolRegistry::with_default_factories()` pre-seeds all four factories; `ToolDiscovery::from_env().discover_into(&mut registry)` loads YAML capabilities; MinIO client initialized if `MINIO_ENDPOINT` is set.
3. Router assembled: public (`/health`, `/v1/files/{token}`) + protected (everything else behind tenant middleware).
4. Layers applied: CORS → `TraceLayer` → tenant extraction → trace propagation.
5. `axum::serve` on `CONUSAI_SERVER__HOST:CONUSAI_SERVER__PORT`.

### Request lifecycle

```
HTTP request
  └─► axum router
        ├─ public_router  ──► /health, /v1/files/{token}
        └─ protected_router (tenant middleware → ResolvedTenant)
              ├─ /v1/chat/completions      → routes/chat.rs    (Rig agent.prompt)
              ├─ /v1/agent/completions     → routes/agent.rs   (≤5-round tool loop)
              │     ├─ if workspace_node_id: WorkspaceStore::get_accessible_node
              │     │     ├─ resolve metadata.thread_id (lazy create + bind_thread)
              │     │     └─ ContextBuilder::build_for_node → system preamble suffix
              │     └─ Anthropic /v1/messages
              │           ├─ on stop_reason=tool_use:
              │           │     registry.get_provider(cap_name)?.invoke(tool, input, tenant)
              │           │       ├─ chain  → InvoiceProvider / ContractProvider / OcrProvider
              │           │       ├─ wasm   → WasmProvider (WasmToolLoader)
              │           │       ├─ mcp    → McpProvider (McpAdapter)
              │           │       └─ native → BuiltinProvider (fs, cargo)
              │           └─ on stop_reason=end_turn:
              │                 ├─ ThreadStore::append(assistant)
              │                 └─ if workspace_node_id:
              │                       WorkspaceStore::index_content(last 30 msgs)
              ├─ /v1/capabilities          → registry list
              ├─ /v1/capabilities/search   → Qdrant (fallback: local)
              ├─ /mcp                      → JSON-RPC dispatcher
              ├─ /v1/files (POST)          → MinIO upload + token
              ├─ /v1/threads               → ThreadStore CRUD (5 routes)
              ├─ /v1/audit                 → AuditStore::list (Qdrant order_by timestamp desc)
              └─ /v1/workspaces            → WorkspaceStore + WorkspaceContentStore (9 routes)
                    ├─ POST    create (folder/conversation; conversation also writes empty .md to MinIO)
                    ├─ GET     tree?parent_id=
                    ├─ GET     search?q=&limit=  (text_match + substring fallback)
                    ├─ GET     {id}
                    ├─ GET/PATCH {id}/content   (PATCH: MinIO write → index_content)
                    ├─ POST    {id}/move         (patch_payload: parent_id + virtual_path)
                    ├─ POST    {id}/share        (owner-only; patch_payload: shared_with)
                    ├─ POST    {id}/unshare      (owner-only; patch_payload: shared_with)
                    └─ DELETE  {id}              (recursive; MinIO best-effort cleanup for conversations)
```

### Tenant propagation

- Middleware decodes JWT (or reads `X-Tenant-ID` in dev), constructs `TenantContext`, inserts as Axum extension.
- Handlers receive it via `Extension(ResolvedTenant)` and pass it through to provider `invoke()` calls, `InvoicePipeline`, etc.
- All filesystem paths via `TenantContext::safe_path`; all object keys prefixed `tenants/{tenant_id}/`; Qdrant collections named `{kind}_{tenant_id}`; spans tagged with tenant fields.

### Rate limiting

Per-tenant 60-second sliding window; plan-based limits (Free 10 / Pro 60 / Enterprise 600 RPM); 429 on exceed.

---

## 7. HTTP API Surface

### Public

| Method | Path | Purpose |
|---|---|---|
| GET | `/health` | Status / version / capability count |
| GET | `/v1/files/{token}` | Token-gated download (1 h TTL) |

### Protected (JWT or `X-Tenant-ID`)

| Method | Path | Purpose |
|---|---|---|
| POST | `/v1/chat/completions` | OpenAI-compatible chat (SSE optional) |
| POST | `/v1/agent/completions` | Tool-calling agent loop |
| GET | `/v1/capabilities` | List capabilities |
| GET | `/v1/capabilities/search?q=&limit=` | Semantic search (Qdrant + fallback) |
| POST | `/mcp` | MCP JSON-RPC 2.0 |
| POST | `/v1/files` | Multipart upload (MinIO) |
| POST | `/v1/threads` | Create thread (optional initial messages + metadata) |
| GET | `/v1/threads` | List threads newest-first (`?limit=20`) |
| GET | `/v1/threads/{thread_id}` | Get thread metadata |
| GET | `/v1/threads/{thread_id}/messages` | Get messages ordered by seq |
| POST | `/v1/threads/{thread_id}/messages` | Append a message |
| GET | `/v1/audit?limit=` | List recent audit events for the calling tenant (newest first; default 50, max 500) |
| POST | `/v1/workspaces` | Create folder or conversation (`{kind, name, parent_id?}`) |
| GET | `/v1/workspaces/tree?parent_id=` | Immediate children visible to caller |
| GET | `/v1/workspaces/search?q=&limit=` | Token-based text_match across `name` ∪ `content_text`, with substring fallback |
| GET | `/v1/workspaces/{id}` | Single node (`NotFound` if not accessible) |
| GET | `/v1/workspaces/{id}/content` | Read markdown body from MinIO |
| PATCH | `/v1/workspaces/{id}/content` | Save body — MinIO write then `index_content` Qdrant payload SET |
| POST | `/v1/workspaces/{id}/move` | Reparent (`{new_parent_id?, new_parent_path?}`) |
| POST | `/v1/workspaces/{id}/share` | Owner-only — append a `user_id` to `shared_with` |
| POST | `/v1/workspaces/{id}/unshare` | Owner-only — remove a `user_id` from `shared_with` |
| DELETE | `/v1/workspaces/{id}` | Recursive (worklist); best-effort MinIO cleanup for conversations |

### Sample payloads

`POST /v1/chat/completions`
```json
{
  "model": "claude-opus-4-7",
  "messages": [
    {"role": "system", "content": "You are helpful."},
    {"role": "user",   "content": "Summarize the attached invoice."}
  ],
  "max_tokens": 2048,
  "stream": true
}
```

`POST /mcp`
```json
{ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }
```

`POST /v1/files` (multipart) →
```json
{
  "id": "a1b2c3d4-…",
  "filename": "invoice.png",
  "size": 245680,
  "tenant_id": "acme",
  "download_url": "/v1/files/a1b2c3d4-…"
}
```

---

## 8. Configuration & Environment

| Var | Default | Purpose |
|---|---|---|
| `CONUSAI_SERVER__HOST` | `0.0.0.0` | Bind address |
| `CONUSAI_SERVER__PORT` | `8080` | Listen port |
| `CONUSAI_CAPABILITIES_DIR` | `./capabilities` | Capability discovery root |
| `CONUSAI_WORKSPACE_ROOT` | `/tmp/conusai/workspaces` | Tenant workspace root |
| `QDRANT_URL` | `http://localhost:6333` | Qdrant REST |
| `MINIO_ENDPOINT` | — | MinIO S3 endpoint (enables file routes) |
| `MINIO_BUCKET` | `conusai` | Bucket |
| `MINIO_ACCESS_KEY` / `MINIO_SECRET_KEY` | `minioadmin` / `minioadmin` | Dev creds |
| `ANTHROPIC_API_KEY` | — | Required for Claude calls |
| `JWT_SECRET` | — | If set: HS256 enforced; if unset: dev mode (`X-Tenant-ID`) |
| `OTLP_ENDPOINT` | — | OTel collector (e.g. `http://localhost:4317`) |
| `RUST_LOG` | — | tracing filter |
| `CONUSAI_TEST_MODE` | — | Set to `1` to replace all Qdrant + MinIO stores with `InMemory*` implementations; no external services required. All data is lost on process exit. |

---

## 9. Build & Deploy

### Local build

```bash
cargo build --release --workspace
cargo build --release --bin agent-gateway
cargo build --release --bin invoice-cli
cargo build --release --bin evals
cargo build --release --target wasm32-wasip1 -p capability-example
```

### Docker

```bash
docker build -t conusai-gateway:latest -t conusai-gateway:0.1.0 .
docker compose --profile full up -d
docker compose --profile infra up -d
docker compose --profile observability up -d
```

### Health

```bash
curl http://localhost:8080/health
curl http://localhost:6333/health
curl http://localhost:9000/minio/health/live
```

---

## 10. Tests & Quality

- **Common (22 tests):** path traversal, safe joins, MCP serialization, `ApiError`, limit invariants, thread/message/tool-call serde roundtrips, `WorkspaceNode` serde + every `validate_name` branch (empty, >255, `/`, `..`, leading `.`, `.md` requirement, happy path), `join_virtual_path` root + nested, `effective_user_id` dev-mode mapping.
- **agent-core (8 tests):** registry register/get/tag-search; manifest embedding text; nonexistent-dir handling; WASM `ping` execution; `QdrantThreadStore` point-id determinism + collection namespacing.
- **Total:** 30 lib tests passing (`cargo test --workspace`). Integration tests under `crates/agent-core/tests/` and `crates/agent-gateway/tests/` exercise live Qdrant; not counted in the lib total.
- **Quality gates:** `cargo clippy --workspace -- -D warnings`, `cargo fmt --all`.

---

## 11. Design Patterns

- **Multitenant-first:** JWT auth, tenant-prefixed paths/keys, Qdrant collection per tenant, plan-based rate limits, tenant-tagged spans.
- **Zero-code extension:** YAML manifests in `capabilities/`; `ToolKind` enum + `ToolProvider` trait allow pluggable execution without touching the registry or agent loop; tool defs in stable `capability__tool` form.
- **Precise tool descriptions drive correct capability selection:** Rich `description` fields in `capability.toml` — loaded verbatim into Anthropic tool definitions — are the primary mechanism for deterministic routing between specialized and generic capabilities (e.g. `invoice-processing` vs `ocr-service`). No code-level classifier needed.
- **Agent loop:** Anthropic `tool_use` with bounded rounds (≤5), accumulating usage on the request span. Thread-aware: loads history, injects summary, persists turns. Supports both blocking JSON and SSE streaming with live `tool_call_start` / `tool_call_result` events.
- **Persistent memory:** `ThreadStore` trait + `QdrantThreadStore` (Qdrant as doc store); one collection per tenant; auto-summarisation via background task when message count crosses threshold.
- **ToolProvider + ToolProviderFactory traits:** `ToolProvider` (`manifest()`, `invoke()`, `invoke_typed<I,O>`, `tool_definitions()`) implemented by `BuiltinProvider`, `McpProvider`, `WasmProvider`, `InvoiceProvider`, `ContractProvider`, `OcrProvider`. `ToolProviderFactory` (`supports(kind, name)`, `create(card)`) implemented by `BuiltinFactory`, `McpFactory`, `WasmFactory`, `ChainFactory`. `ToolRegistry::with_default_factories()` pre-registers all four. Adding a new capability kind requires one new provider file + one factory struct — zero changes to the registry, executor, or agent loop.
- **Native tools:** `ToolKind::Native` + `tools/builtin/` module (`BuiltinProvider`) — filesystem (read/write) and cargo runner available to any agent turn; path-safety enforced via `safe_join`.
- **Observability by default:** structured JSON logs, OTel spans with W3C context propagation, healthchecks at every layer.

---

## 12. Security

- **Authentication:** HS256 JWT in production (`JWT_SECRET`); dev fallback via `X-Tenant-ID` header.
- **Path safety:** `safe_join` rejects `..`; all tenant FS access via `TenantContext::safe_path`.
- **Storage isolation:** MinIO keys under `tenants/{tenant_id}/`; download tokens with 1 h TTL.
- **Vector isolation:** Qdrant collection per tenant.
- **Secrets:** read from environment; `.env`/`.env.local` are git-ignored.
- **WASM sandboxing:** Wasmtime engine, `MAX_WASM_SIZE_BYTES = 10 MB`, only whitelisted exports invoked.

---

## 13. Status

- **Version:** 0.1.0
- **State:** operational, ~95 % verified end-to-end (per [verify.md](../verify.md)).

**Implemented:** multitenancy, invoice + contract pipelines, YAML capability discovery, OpenAI-compatible chat, SSE streaming, tool-calling agent loop (blocking + streaming), MCP JSON-RPC, Qdrant semantic capability search, MinIO file storage, WASM execution, Google Workspace manifest, evals framework (invoice + OCR + threads), Jaeger/OTLP tracing, per-tenant rate limiting, persistent thread memory (Qdrant-backed) with auto-summarisation, thread REST API (5 endpoints), `gen_ai.*` OTel span attributes, W3C traceparent propagation, native filesystem + cargo tools, cargo-chef Docker caching, **hierarchical workspace** (folders + conversations as `.md` in MinIO; per-tenant Qdrant index with text indexes on `name` + `content_text`; private-by-default per-user ACL with explicit per-node sharing — see [ADR 005](adr/005-workspace-access-control.md)), **per-node thread binding** (lazy `bind_thread` from agent route), **chat-content indexing** (last 30 messages re-indexed into `content_text` after every turn), **workspace context injection** (`ContextBuilder` walks ancestor `CONTEXT.md` / `README.md`), **append-only audit log** (`audit_{tenant_id}` Qdrant collection + `GET /v1/audit`), **workspace-first sidebar redesign** (Workspace + search + tree, Recents, Capabilities, user chip), `metrics` module with OTel meters for Qdrant + LLM operations, **`Capability*` → `Tool*` refactor** (`ToolProvider` trait; provider-based registry; `BuiltinProvider`/`McpProvider`/`WasmProvider`/`InvoiceProvider`/`ContractProvider`/`OcrProvider`; shared `QdrantClient` helper; `ExtractionPipeline` trait), **`Pipeline` → `Chain` refactor (plan.md v0.2.0)** (`ToolKind::Chain` / `kind: chain` wire format; `src/pipelines/` → `src/chains/`; `ExtractionPipeline::run(bytes, tenant)` primary method; `ToolProviderFactory` trait + four factory structs (`McpFactory`, `WasmFactory`, `ChainFactory`, `BuiltinFactory`); `ToolRegistry::with_default_factories()` / `with_builtin()`; `ToolDiscovery::discover_into()`; `invoke_typed<I,O>` + `invoke_typed_dyn`; `error.rs` `Rig`/`WasmRuntime`/`Qdrant` variants; `executor.rs` span carries `tenant_id`; telemetry fix — single `SdkMeterProvider` for Prometheus + OTLP, no duplicate-registry panic).

**Reserved / future:** `Docker` capability kind, external MCP server federation, multi-instance deployment, audit retention/compaction, billing/quota enforcement, admin dashboard, multi-layer context budgeting (plan §7), live document mode (plan §8), agent-callable workspace toolkit (plan §9), real workspace embeddings (vectors are still 4-dim placeholders — see plan §9 / future ADR 006).

---

## 14. File-Tree Summary

```
conusai-platform/
├── Cargo.toml                       # workspace
├── Dockerfile                       # multi-stage gateway image
├── docker-compose.yml               # qdrant, minio, gateway, jaeger, otel-collector
├── start.sh                         # orchestration entrypoint
├── rust-toolchain.toml              # stable + wasm32-wasip1
├── docs/                            # arch.md, plan.md, tenant.md, verify.md, ui-design.md, adr/
│
├── crates/
│   ├── common/        src/{lib,error,config/mod,telemetry,http_client,mcp,wasm,limits,path_safety,eval,
│   │                       audit,metrics,
│   │                       memory/{mod,thread,store,workspace,tests}}.rs
│   ├── agent-core/    src/{lib,
│   │                       agent/{mod,builder,runtime},
│   │                       context/{mod,tenant},
│   │                       memory/{mod,qdrant_helpers,qdrant_store,qdrant_workspace_store,
│   │                               minio_workspace_content,context_builder,qdrant_audit},
│   │                       tools/{mod,provider,manifest,card,registry,discovery,
│   │                              embedding,executor,mcp_adapter,wasm_loader,
│   │                              providers/{mod,builtin,mcp,wasm,chain},
│   │                              builtin/{mod,fs,cargo,card}},
│   │                       chains/{mod,extraction,invoice,contract}}.rs
│   ├── agent-gateway/ src/{main,state,
│   │                       mw/{mod,tenant,trace,rate_limit},
│   │                       routes/{mod,health,chat,agent,capabilities,search,mcp,files,
│   │                               threads,audit,workspaces},
│   │                       ui/{mod,routes,session,view,
│   │                           handlers/{mod,auth,app,chat,upload,invoice}}}.rs
│   │                   assets/
│   │                       css/style.css          ← design system + workspace styles (~1320 lines)
│   │                       js/app.js              ← streaming + composer + ws:select handler (~660 lines)
│   │                       js/workspace.js        ← tree + search + dialogs + ctx menu (~750 lines)
│   │                       icons/icons.svg        ← SVG sprite
│   │                       images/{favicon.png,conusai-logo-{light,dark}mode.png}
│   │                   templates/{app,login}.html
│   │                       partials/composer.html
│   │                       shared/head.html
│
├── examples/
│   └── invoice-cli/   src/main.rs
│
├── capabilities/
│   ├── file-storage/        capability.toml         (mcp)
│   ├── google-workspace/    capability.toml         (mcp)
│   ├── contract-processing/ capability.toml         (chain)
│   ├── invoice-processing/  capability.toml         (chain)
│   ├── ocr-service/         capability.toml         (chain)
│   ├── template-wasm/       capability.toml + .wasm (wasm)
│   └── template/                                    (boilerplate)
│
├── evals/
│   ├── src/{main,config,report,
│   │        runners/{mod,invoice,ocr_quality},
│   │        scorers/mod}.rs
│   └── datasets/{invoice,ocr_quality}.jsonl
│
├── wasm/                            # WASM capability sources (reserved)
├── scripts/
│   ├── docker-verify.sh
│   └── otel-collector.yaml
└── docs/
    ├── arch.md                      # this document — master index
    ├── plan.md                      # workspace implementation plan + phase status
    ├── tenant.md                    # multitenancy design
    ├── verify/verify.md             # end-to-end verification plan
    ├── ui-design.md                 # design tokens + component recipes
    └── adr/
        └── 005-workspace-access-control.md
```
