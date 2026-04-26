# ConusAI Platform — Architecture & Functionality

A production-grade multitenant AI agent platform built on **Rust + Rig**, with **WASM/MCP capabilities**, **Qdrant** vector search, **MinIO** file storage, and full **OpenTelemetry** observability.

---

## 1. Workspace Overview

### Workspace Members ([Cargo.toml](../Cargo.toml))

- [crates/common](../crates/common) — Shared utilities and foundational types
- [crates/agent-core](../crates/agent-core) — Agent runtime, capability registry, Rig integration
- [crates/agent-gateway](../crates/agent-gateway) — OpenAI-compatible HTTP gateway
- [crates/invoice-demo](../crates/invoice-demo) — Standalone invoice extraction CLI
- [evals](../evals) — Evaluation framework (runners + scorers)

### Key Workspace Dependencies

| Category | Crates |
|----------|--------|
| Async runtime | `tokio` (full), `tokio-stream` |
| AI / LLM | `rig-core` 0.9 (Anthropic) |
| Vector DB | `qdrant-client` 1.x |
| HTTP server | `axum` 0.8 (ws, multipart), `tower`, `tower-http` (cors, trace, br) |
| HTTP client | `reqwest` 0.12 (json, stream) |
| Serialization | `serde`, `serde_json`, `serde_yaml` |
| Config | `figment` (env, toml, yaml) |
| Errors | `thiserror`, `anyhow` |
| Observability | `tracing`, `tracing-subscriber`, `opentelemetry` 0.27, `opentelemetry-otlp`, `tracing-opentelemetry` |
| WASM | `wasmtime` 29 (component-model), `wasmtime-wasi` |
| Auth/Crypto | `jsonwebtoken`, `sha2`, `hmac` |
| Schema | `schemars` 0.8 |
| Object storage | `object_store` 0.11 (S3/MinIO) |
| IDs | `ulid = "1.1"` (time-sortable, serde) |
| Utilities | `uuid`, `chrono`, `bytes`, `futures`, `async-trait`, `bon`, `clap`, `colored` |

- **Rust edition:** 2024
- **Dependency resolver:** 3
- **Rust version:** 1.88 (stable)
- **WASM target:** `wasm32-wasip1` (configured in [rust-toolchain.toml](../rust-toolchain.toml))

---

## 2. Top-Level Files

### [Dockerfile](../Dockerfile) — 4-stage cargo-chef build

**Stage 1 — Planner** (`rust:1.88-slim`) — `cargo chef prepare` generates `recipe.json` from the workspace manifests.

**Stage 2 — Cacher** — `cargo chef cook --release` builds all dependencies into a layer; cached by Docker unless `Cargo.toml`/`Cargo.lock` change (10× faster incremental rebuilds).

**Stage 3 — Builder** — copies pre-built deps from cacher, compiles real source with `cargo build --release --bin agent-gateway`.

**Stage 4 — Runtime** (`debian:bookworm-slim`)
- Installs `libssl3`, `ca-certificates`, `curl`
- Copies `/build/target/release/agent-gateway` → `/app/agent-gateway`
- Copies `/capabilities` (read-only)
- Env: `CONUSAI_SERVER__HOST=0.0.0.0`, `CONUSAI_SERVER__PORT=8080`, `CONUSAI_CAPABILITIES_DIR=/app/capabilities`
- Exposes 8080
- Healthcheck: `curl -sf http://localhost:8080/health` (15s/5s/3 retries)

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
| [paln.md](../paln.md) | Detailed phased implementation plan (init → common → agent-core → capabilities → evals → gateway → infra → polish). |
| [tenant.md](../tenant.md) | Multitenancy design — JWT, path scoping, Qdrant collection namespacing, rate limiting. |
| [verify.md](../verify.md) | End-to-end Docker verification plan (~95% coverage), JWT helpers, curl recipes. |

---

## 3. Crates

### 3.1 [`crates/common`](../crates/common) — Shared Utilities

**Purpose:** foundational types, errors, telemetry, MCP JSON-RPC 2.0, WASM loader, config, path safety.

| File | Purpose |
|---|---|
| [src/lib.rs](../crates/common/src/lib.rs) | Re-exports modules; defines `prelude` (`Result`, `ConusAiError`). |
| [src/error.rs](../crates/common/src/error.rs) | `ConusAiError` enum (Config / Capability / Wasm / Mcp / Api / Io / Other). `ApiError { code, message }`. |
| [src/config/mod.rs](../crates/common/src/config/mod.rs) | `AppConfig`, `ServerConfig`, `QdrantConfig`, `TelemetryConfig`. Layered loading via `figment` (TOML + env + YAML). |
| [src/telemetry.rs](../crates/common/src/telemetry.rs) | `TelemetryGuard` (RAII). `init(name, level)` — JSON `tracing-subscriber` + optional OTLP/Jaeger span exporter. |
| [src/http_client.rs](../crates/common/src/http_client.rs) | `build_client()` → `reqwest::Client` (60 s timeout, UA `conusai-platform/0.1`). |
| [src/mcp.rs](../crates/common/src/mcp.rs) | `JsonRpcRequest` / `JsonRpcResponse` / `JsonRpcError` (jsonrpc 2.0). |
| [src/wasm.rs](../crates/common/src/wasm.rs) | `WasmLoader` wrapping `wasmtime::Engine`: `load_bytes`, `load_file`, `new_store`. |
| [src/limits.rs](../crates/common/src/limits.rs) | Constants: `MAX_PROMPT_TOKENS=128k`, `MAX_RESPONSE_TOKENS=16k`, `MAX_CAPABILITY_SIZE_BYTES=50 MB`, `MAX_WASM_SIZE_BYTES=10 MB`, `REQUEST_TIMEOUT_SECS=120`, `MAX_CONCURRENT_AGENTS=64`, `MAX_MESSAGES_PER_THREAD=10_000`, `MAX_MESSAGES_BEFORE_SUMMARY=50`. |
| [src/path_safety.rs](../crates/common/src/path_safety.rs) | `safe_join()` (rejects `..`), `join_under_tenant(root, tenant_id, rel)`. |
| [src/eval.rs](../crates/common/src/eval.rs) | Trait stubs shared by the evals crate. |
| [src/memory/thread.rs](../crates/common/src/memory/thread.rs) | `Thread { id (ULID), tenant_id, title, created_at, last_active, message_count, summary, metadata }`. `Message { role, content, tool_calls, timestamp, seq }`. `ToolCall { id, name, input, output }`. |
| [src/memory/store.rs](../crates/common/src/memory/store.rs) | `ThreadStore` async trait: `create`, `get`, `messages`, `append`, `list`, `set_summary`, `set_title`. All methods take `tenant_id: &str` (avoids circular dep with agent-core). |

**Tests:** path-traversal rejection, valid joins, MCP serialization, `ApiError` fields, limit invariants, thread/message/tool-call serde roundtrips.

---

### 3.2 [`crates/agent-core`](../crates/agent-core) — Agent Runtime & Capability Registry

**Purpose:** Rig integration; capability discovery / registration; tool execution (MCP, WASM, pipeline); tenant context; invoice pipeline.

#### Capabilities subsystem ([src/capabilities/](../crates/agent-core/src/capabilities))

| File | Purpose |
|---|---|
| [mod.rs](../crates/agent-core/src/capabilities/mod.rs) | Re-exports `card`, `discovery`, `embedding`, `manifest`, `mcp_adapter`, `provider`, `registry`, `tool_executor`, `wasm_loader`. |
| [provider.rs](../crates/agent-core/src/capabilities/provider.rs) | `AgentCapability` async trait — `name()`, `description()`, `tool_names()`, `invoke(tool, input) -> Value`. |
| [manifest.rs](../crates/agent-core/src/capabilities/manifest.rs) | `CapabilityManifest { name, version, description, kind, tools, config, tags }`; `CapabilityKind { Mcp, Wasm, Pipeline, Docker, Native }`; `ToolDef { name, description, input_schema }`. `from_yaml`, `from_file`, `embedding_text`. |
| [card.rs](../crates/agent-core/src/capabilities/card.rs) | `CapabilityCard { id (UUID), manifest, source_path, embedding_id }`. |
| [registry.rs](../crates/agent-core/src/capabilities/registry.rs) | `CapabilityRegistry`: in-memory `HashMap<String, CapabilityCard>`. `register`, `get`, `search_by_tag`, `all`, `len`, `is_empty`, `load_from_dir(dir)` — auto-discovers any subdir containing `capability.yaml`. |
| [discovery.rs](../crates/agent-core/src/capabilities/discovery.rs) | `CapabilityDiscovery` — `from_env()` reads `CONUSAI_CAPABILITIES_DIR` (default `./capabilities`); `discover()` returns a populated `CapabilityRegistry`. |
| [embedding.rs](../crates/agent-core/src/capabilities/embedding.rs) | `ToolEmbedding::describe(card)` — returns text used for semantic search (delegates to `manifest.embedding_text()`). |
| [tool_executor.rs](../crates/agent-core/src/capabilities/tool_executor.rs) | `CapabilityExecutor::invoke(card, tool, input, tenant)` dispatcher: routes `invoice-processing` / `ocr-service` to `InvoicePipeline`, `native-tools` to `crate::tools`, WASM caps to `WasmCapabilityLoader`. Builds Anthropic tool definitions in `capability__tool` form via `tool_definitions(card)`. Downloads URL images to temp files when needed. |
| [mcp_adapter.rs](../crates/agent-core/src/capabilities/mcp_adapter.rs) | `McpAdapter` — JSON-RPC 2.0 HTTP client (`call`, `list_tools`, `call_tool`) for external MCP servers. |
| [wasm_loader.rs](../crates/agent-core/src/capabilities/wasm_loader.rs) | `WasmCapabilityLoader` (wraps `wasmtime::Engine`). `load(card)` reads `card.source_path/capability.wasm`; `invoke_i32`, `invoke_tool(card, tool, input)` (currently dispatches `ping`). |

#### Agent subsystem ([src/agent/](../crates/agent-core/src/agent))

| File | Purpose |
|---|---|
| [builder.rs](../crates/agent-core/src/agent/builder.rs) | `GeneralAgentBuilder` (fluent — `model`, `preamble`, `max_tokens`, `with_tenant`, `build`); `build_for_tenant`. Honors plan-based `max_tokens`. `GeneralAgent::prompt(text)` wraps the Rig Anthropic agent. |
| [runtime.rs](../crates/agent-core/src/agent/runtime.rs) | `AgentRuntime` = `GeneralAgent` + `CapabilityRegistry`; `new`, `for_tenant`, `run`, `registry`. |

#### Context subsystem ([src/context/](../crates/agent-core/src/context))

| File | Purpose |
|---|---|
| [tenant.rs](../crates/agent-core/src/context/tenant.rs) | `PlanTier { Free, Pro, Enterprise }` with `max_tokens()` (4k/16k/128k) and `rate_limit_rpm()` (10/60/600). `TenantContext { tenant_id, user_id, plan, workspace_root }` with `tenant_root()`, `safe_path(rel)`, `storage_prefix()` (`tenants/{id}/`), `qdrant_collection(kind)` (`{kind}_{tenant_id}`), `span_fields()`. `TenantClaims { sub, tenant_id, plan, exp }` for JWT. |
| [mod.rs](../crates/agent-core/src/context/mod.rs) | Also exposes `ConversationContext` for chat history. |

#### Pipelines ([src/pipelines/](../crates/agent-core/src/pipelines))

| File | Purpose |
|---|---|
| [invoice.rs](../crates/agent-core/src/pipelines/invoice.rs) | `InvoiceLineItem`, `InvoiceData` (~20 fields). `InvoicePipeline::new()` (default `claude-opus-4-7`), `with_model`, `with_tenant`, `extract_from_image_path`, `extract_from_bytes` — base64-encodes image, sends to Claude vision with strict JSON schema prompt, parses to `InvoiceData`. |

#### Memory subsystem ([src/memory/](../crates/agent-core/src/memory))

| File | Purpose |
|---|---|
| [qdrant_store.rs](../crates/agent-core/src/memory/qdrant_store.rs) | `QdrantThreadStore` — implements `ThreadStore` using Qdrant REST as a document store (not vector search). Uses 4-dim zero vectors; SHA-256 → u64 point IDs; collection per tenant (`threads_{tenant_id}`); payload indices on `type`, `thread_id`, `tenant_id`. `scroll_filter()` for all queries. Background `tokio::spawn` for auto-summarisation when `message_count % MAX_MESSAGES_BEFORE_SUMMARY == 0`, calling Claude Haiku via Anthropic API. All 7 trait methods instrumented with `#[instrument]` (OTel spans). |

#### Native tools subsystem ([src/tools/](../crates/agent-core/src/tools))

| File | Purpose |
|---|---|
| [fs_tools.rs](../crates/agent-core/src/tools/fs_tools.rs) | `read_file(workspace_root, input)` / `write_file(workspace_root, input)` — tenant-scoped filesystem access via `safe_join` (rejects `..`). Uses `tokio::fs`. |
| [cargo_tool.rs](../crates/agent-core/src/tools/cargo_tool.rs) | `run_cargo(workspace_root, input)` — runs `cargo {check,test,build,clippy,fmt}` via `tokio::process::Command`; returns stdout/stderr/exit_code as JSON. Allowlisted subcommands only. |
| [native_capability.rs](../crates/agent-core/src/tools/native_capability.rs) | `native_capability_card()` — builds a `CapabilityCard` with `kind: Native` exposing `read_file`, `write_file`, `run_cargo` with full JSON schemas. Auto-registered at gateway startup. |

**Public re-exports** (via [`lib.rs`](../crates/agent-core/src/lib.rs)): `GeneralAgent`, `GeneralAgentBuilder`, `CapabilityDiscovery`, `CapabilityRegistry`, `PlanTier`, `TenantClaims`, `TenantContext`, `InvoiceData`, `InvoiceLineItem`, `InvoicePipeline`, `native_capability_card`.

---

### 3.3 [`crates/agent-gateway`](../crates/agent-gateway) — HTTP API

**Purpose:** OpenAI-compatible chat/agent endpoints, tool calling, MCP dispatch, capability search, file upload/download, JWT auth, rate limiting.

| File | Purpose |
|---|---|
| [src/main.rs](../crates/agent-gateway/src/main.rs) | Tokio entrypoint. Initializes telemetry, builds `AppState`, mounts public + protected routers, applies `CorsLayer` + `TraceLayer` + tenant + trace middleware, binds `0.0.0.0:8080`. |
| [src/state.rs](../crates/agent-gateway/src/state.rs) | `AppState { registry: Mutex<CapabilityRegistry>, rate_limiter, file_store: Option<Arc<dyn ObjectStore>>, thread_store: Arc<dyn ThreadStore>, qdrant_url, presigned_tokens: Mutex<HashMap> }`. `from_env()` runs capability discovery, registers `native_capability_card()`, initializes MinIO if `MINIO_ENDPOINT` is set, and initializes `QdrantThreadStore`. |

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
| [agent.rs](../crates/agent-gateway/src/routes/agent.rs) | `POST /v1/agent/completions` | Thread-aware tool-calling agent loop with blocking and streaming (`"stream": true`) modes. Accepts optional `thread_id`; loads history + injects thread summary as system context; persists user message before loop and assistant reply after; auto-sets thread title from first reply. Up to 5 tool-use rounds. Streaming path emits OpenAI SSE chunks + `tool_call_start` / `tool_call_result` events so clients can follow tool execution in real-time. Accumulates `gen_ai.*` span attributes (model, input/output tokens). Returns `thread_id` in response. |
| [threads.rs](../crates/agent-gateway/src/routes/threads.rs) | Thread CRUD | `create_thread`, `list_threads`, `get_thread`, `get_messages`, `append_message`. Delegates to `AppState::thread_store` (`QdrantThreadStore`). |
| [capabilities.rs](../crates/agent-gateway/src/routes/capabilities.rs) | `GET /v1/capabilities` | Lists capabilities (name, version, description, kind, tags, tools) with tenant + plan. |
| [search.rs](../crates/agent-gateway/src/routes/search.rs) | `GET /v1/capabilities/search?q=…&limit=…` | Semantic search via Qdrant (64-dim deterministic hash embeddings). On first call per tenant, creates collection `capabilities_{tenant_id}` and upserts capability vectors. Falls back to local substring match if Qdrant is unreachable. |
| [mcp.rs](../crates/agent-gateway/src/routes/mcp.rs) | `POST /mcp` | JSON-RPC 2.0 dispatcher. Methods: `initialize` (server info), `tools/list` (all tool defs), `tools/call` (`capability__tool`, splits name, dispatches to `CapabilityExecutor`). |
| [files.rs](../crates/agent-gateway/src/routes/files.rs) | `POST /v1/files`, `GET /v1/files/{token}` | Multipart upload to MinIO under `tenants/{tenant_id}/{uuid}/{filename}`; returns 1-h TTL download token. Download endpoint is public (token-gated) and streams back the object. |
| [mod.rs](../crates/agent-gateway/src/routes/mod.rs) | `public_router()` (health + file download), `protected_router()` (everything else, with tenant middleware + 5 thread routes). |

---

### 3.4 [`crates/invoice-demo`](../crates/invoice-demo) — Standalone CLI

[`main.rs`](../crates/invoice-demo/src/main.rs):

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

Drop a folder with a `capability.yaml` (and optionally an implementation) into `capabilities/`; the registry auto-discovers it on startup.

### Capability kinds (`CapabilityKind`)

| Kind | Runtime | Implementation | Tool format |
|---|---|---|---|
| `mcp` | External process | JSON-RPC 2.0 over HTTP / stdio | MCP standard |
| `wasm` | Wasmtime | `wasm32-wasip1` module | Exported WASM functions |
| `pipeline` | In-process Rig | Claude vision + structured extraction | Rig agent + tool defs |
| `docker` | Container | (reserved / future) | TBD |
| `native` | In-process Rust | `crate::tools` (fs, cargo) | Built-in — no YAML manifest |

### Discovered capabilities

| Folder | Kind | Tools | Notes |
|---|---|---|---|
| [file-storage](../capabilities/file-storage/capability.yaml) | mcp | `upload_file`, `download_file`, `presigned_url` | Manifest only — actual storage handled directly by [`routes/files.rs`](../crates/agent-gateway/src/routes/files.rs) using `object_store`. |
| [google-workspace](../capabilities/google-workspace/capability.yaml) | mcp | `list_files`, `read_document`, `append_to_sheet`, `send_email` | OAuth2 scopes: `drive.readonly`, `documents.readonly`, `spreadsheets`, `gmail.send`. |
| [invoice-processing](../capabilities/invoice-processing/capability.yaml) | pipeline | `extract_invoice`, `validate_invoice` | Backed by [`InvoicePipeline`](../crates/agent-core/src/pipelines/invoice.rs); default model `claude-opus-4-7`, max image 20 MB, formats `png/jpeg/jpg/pdf`. |
| [ocr-service](../capabilities/ocr-service/capability.yaml) | pipeline | `extract_text` | Reuses `InvoicePipeline` for vision OCR; default model `claude-sonnet-4-6`. |
| [template-wasm](../capabilities/template-wasm/capability.yaml) | wasm | `ping` | Loads `capability.wasm` exporting `ping() -> i32 = 42`. |
| [template](../capabilities/template) | — | — | Boilerplate for new capabilities. |

### Capability selection: `invoice-processing` vs `ocr-service`

These two capabilities are intentionally **non-overlapping** — the LLM (Claude) selects the right one via tool description quality and Qdrant semantic embeddings:

| Need | Correct capability |
|---|---|
| Invoice, bill, purchase order, accounts-payable document → **structured fields** | `invoice-processing__extract_invoice` |
| Contract, letter, handwritten note, generic document → **raw text** | `ocr-service__extract_text` |

`invoice-processing__extract_invoice` handles the vision step internally (Claude vision + strict JSON schema in one call). Calling `ocr-service` before it is redundant and adds unnecessary latency. The rich `description` fields in both `capability.yaml` files — loaded verbatim into tool definitions at startup — make this routing deterministic without any code-level classifier.

---

## 5. Other Top-Level Folders

### [`wasm/`](../wasm)
Reserved for WASM capability source crates targeting `wasm32-wasip1`.

### [`scripts/`](../scripts)

| File | Purpose |
|---|---|
| [docker-verify.sh](../scripts/docker-verify.sh) | Automated end-to-end Docker verification (per [verify.md](../verify.md)). |
| [otel-collector.yaml](../scripts/otel-collector.yaml) | OTel Collector config — OTLP gRPC/HTTP receivers, Jaeger exporter. |

---

## 6. Runtime Flow

### Startup (gateway)

1. `tokio::main` → `common::telemetry::init("agent-gateway", "info")` (JSON logs + optional OTLP).
2. `AppState::from_env()` → `CapabilityDiscovery::from_env().discover()` populates `CapabilityRegistry`; MinIO client initialized if `MINIO_ENDPOINT` is set.
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
              │     └─ Anthropic /v1/messages
              │           └─ on stop_reason=tool_use:
              │                 CapabilityExecutor::invoke(card, tool, input, tenant)
              │                   ├─ pipeline → InvoicePipeline
              │                   ├─ wasm     → WasmCapabilityLoader
              │                   └─ mcp      → McpAdapter
              ├─ /v1/capabilities          → registry list
              ├─ /v1/capabilities/search   → Qdrant (fallback: local)
              ├─ /mcp                      → JSON-RPC dispatcher
              └─ /v1/files (POST)          → MinIO upload + token
```

### Tenant propagation

- Middleware decodes JWT (or reads `X-Tenant-ID` in dev), constructs `TenantContext`, inserts as Axum extension.
- Handlers receive it via `Extension(ResolvedTenant)` and pass it through to `CapabilityExecutor`, `InvoicePipeline`, etc.
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

---

## 9. Build & Deploy

### Local build

```bash
cargo build --release --workspace
cargo build --release --bin agent-gateway
cargo build --release --bin invoice-demo
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

- **Common:** path traversal, safe joins, MCP serialization, `ApiError`, limit invariants, thread/message/tool-call serde roundtrips.
- **agent-core:** registry register/get/tag-search; manifest embedding text; nonexistent-dir handling; WASM `ping` execution; `QdrantThreadStore` point-id determinism + collection namespacing.
- **Total:** 19 tests passing.
- **Quality gates:** `cargo clippy --workspace -- -D warnings`, `cargo fmt --all`.

---

## 11. Design Patterns

- **Multitenant-first:** JWT auth, tenant-prefixed paths/keys, Qdrant collection per tenant, plan-based rate limits, tenant-tagged spans.
- **Zero-code extension:** YAML manifests in `capabilities/`; `CapabilityKind` enum allows pluggable execution; tool defs in stable `capability__tool` form.
- **Precise tool descriptions drive correct capability selection:** Rich `description` fields in `capability.yaml` — loaded verbatim into Anthropic tool definitions — are the primary mechanism for deterministic routing between specialized and generic capabilities (e.g. `invoice-processing` vs `ocr-service`). No code-level classifier needed.
- **Agent loop:** Anthropic `tool_use` with bounded rounds (≤5), accumulating usage on the request span. Thread-aware: loads history, injects summary, persists turns. Supports both blocking JSON and SSE streaming with live `tool_call_start` / `tool_call_result` events.
- **Persistent memory:** `ThreadStore` trait + `QdrantThreadStore` (Qdrant as doc store); one collection per tenant; auto-summarisation via background task when message count crosses threshold.
- **Native tools:** `CapabilityKind::Native` + `tools/` module — filesystem (read/write) and cargo runner available to any agent turn; path-safety enforced via `safe_join`.
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

**Implemented:** multitenancy, invoice pipeline, YAML capability discovery, OpenAI-compatible chat, SSE streaming, tool-calling agent loop (blocking + streaming), MCP JSON-RPC, Qdrant semantic search, MinIO file storage, WASM execution, Google Workspace manifest, evals framework (invoice + OCR + threads), Jaeger/OTLP tracing, per-tenant rate limiting, persistent thread memory (Qdrant-backed), thread REST API (5 endpoints), thread-aware agent loop with auto-summarisation, `gen_ai.*` OTel span attributes, W3C traceparent propagation, native filesystem + cargo tools, cargo-chef Docker caching.

**Reserved / future:** `Docker` capability kind, external MCP server federation, multi-instance deployment, persistent audit log, billing/quota enforcement, admin dashboard.

---

## 14. File-Tree Summary

```
conusai-platform/
├── Cargo.toml                       # workspace
├── Dockerfile                       # multi-stage gateway image
├── docker-compose.yml               # qdrant, minio, gateway, jaeger, otel-collector
├── start.sh                         # orchestration entrypoint
├── rust-toolchain.toml              # stable + wasm32-wasip1
├── paln.md / tenant.md / verify.md  # design + verification docs
│
├── crates/
│   ├── common/        src/{lib,error,config/mod,telemetry,http_client,mcp,wasm,limits,path_safety,eval,
│   │                       memory/{mod,thread,store,tests}}.rs
│   ├── agent-core/    src/{lib,
│   │                       agent/{mod,builder,runtime},
│   │                       capabilities/{mod,provider,manifest,card,registry,discovery,
│   │                                     embedding,tool_executor,mcp_adapter,wasm_loader},
│   │                       context/{mod,tenant},
│   │                       memory/{mod,qdrant_store},
│   │                       tools/{mod,fs_tools,cargo_tool,native_capability},
│   │                       pipelines/{mod,invoice}}.rs
│   ├── agent-gateway/ src/{main,state,
│   │                       mw/{mod,tenant,trace,rate_limit},
│   │                       routes/{mod,health,chat,agent,capabilities,search,mcp,files,threads}}.rs
│   └── invoice-demo/  src/main.rs
│
├── capabilities/
│   ├── file-storage/        capability.yaml         (mcp)
│   ├── google-workspace/    capability.yaml         (mcp)
│   ├── invoice-processing/  capability.yaml         (pipeline)
│   ├── ocr-service/         capability.yaml         (pipeline)
│   ├── template-wasm/       capability.yaml + .wasm (wasm)
│   └── template/                                    (boilerplate)
│
├── evals/
│   ├── src/{main,config,report,
│   │        runners/{mod,invoice,threads,ocr_quality},
│   │        scorers/mod}.rs
│   └── datasets/{invoice,threads,ocr_quality}.jsonl
│
├── wasm/                            # WASM capability sources (reserved)
├── scripts/
│   ├── docker-verify.sh
│   └── otel-collector.yaml
└── docs/
    └── arch.md                      # this document
```
