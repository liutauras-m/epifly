# ConusAI Platform — Architecture & Functionality

> **Current workspace snapshot (v0.4.0 — May 2026).** Production-grade multitenant AI agent platform built as a **pnpm + Cargo monorepo**. Persistent metadata lives in a single embedded **redb** key-value store, vectors live in **Qdrant**, content lives in **RustFS** (S3-compatible object storage), and the cross-platform client is a **SvelteKit + Tauri** Browser Shell that runs on macOS, Windows, Linux, iOS, and Android.

The monorepo packages:

| Path | Stack | Role |
|---|---|---|
| `apps/backend/` | Rust 1.95, Axum 0.8, Rig 0.36 | Multitenant agent gateway, capability registry, jobs, indexer, OpenAPI/MCP, Foundry UI |
| `apps/web/` | SvelteKit 2, Svelte 5, Node 22 | Public web app (Node adapter); SSR session bridge to gateway |
| `apps/browser-shell/` | SvelteKit + Tauri 2 (Rust) | Cross-platform desktop + mobile shell: tabs, session recorder, device auth |
| `packages/sdk/` | TypeScript | Typed gateway client (`createConusSdk`); endpoints, streaming, types |
| `packages/types/` | TypeScript | Shared domain types mirroring Rust crates (`SessionTrace`, `WorkspaceNode`, `CapabilityCard`, `ControlMessage`) |
| `packages/ui/` | Svelte 5 component library | Design system, stores, capability renderer registry, features (chat, workspace, auth) |
| `services/current-time/` | Python (FastAPI) | Reference self-registering MCP capability service |
| `e2e/` | Playwright + WDIO + Appium | Cross-platform end-to-end suites (web, iOS web, shell-macos, native iOS) |

---

## 1. Technology Stack

### Backend (Rust)

| Layer | Technology | Version |
|---|---|---|
| Language | Rust (edition 2024) | 1.95 stable |
| WASM target | `wasm32-wasip1` + Component Model | wasmtime 44 |
| AI framework | `rig-core` | 0.36 |
| HTTP framework | `axum` (+ `axum-extra`, `tower`, `tower-http`) | 0.8 |
| Async runtime | `tokio` | 1.x (full features) |
| **Metadata store** | **`redb`** (embedded KV, postcard serde) | 2.x |
| **Vector DB** | **`qdrant-client`** (Qdrant 1.17) | 1.x |
| Object storage | `object_store` (AWS feature) → RustFS / MinIO / S3 | 0.11 |
| Embeddings | `fastembed` (feature `local-embeddings`) or OpenAI HTTP | 5.x |
| Auth | `jsonwebtoken`, `hmac`, `sha2`, `blake3` | 9 / 0.12 / 0.10 / 1 |
| Templates | `askama` (gateway UI) | 0.12 |
| OpenAPI | `utoipa` + `utoipa-swagger-ui` | 5 / 9 |
| Observability | `opentelemetry` 0.27 (OTLP + Prometheus) | — |
| Scheduled jobs | `tokio-cron-scheduler` | 0.13 |
| Config | `figment` (TOML + env, prefix `CONUSAI_`) | 0.10 |
| Caching | `moka` (futures) | 0.12 |
| Builder DSL | `bon` | 3 |
| IDs | `ulid`, `uuid` v4/v5 | 1.1 / 1 |
| Errors | `thiserror` 2, `anyhow` 1 | — |
| Wire format | `serde_json` 1, `toml` 0.8, `postcard` 1 | — |

### Frontend (Web + Shell)

| Layer | Technology | Version |
|---|---|---|
| Package manager | **pnpm** (workspaces) | 10.13 |
| Build / orchestration | **Turborepo** | — |
| Linter / formatter | **Biome** | — |
| Web framework | SvelteKit (Node adapter) | 2.21 |
| Shell framework | SvelteKit (Static adapter) inside Tauri 2 | — |
| Component runtime | Svelte | 5.33 (runes) |
| Tauri plugins | `dialog`, `stronghold` (secure device-token storage) | 2.x |
| Type checking | `svelte-check` + `typescript` | 5.7 |
| Unit tests | `vitest` | 2.1 |
| E2E (web/shell-web/ios-web) | `@playwright/test` | ≥ 1.49 |
| E2E (native shells) | `webdriverio` + `appium` + `appium-xcuitest-driver` | 9.x / 3.4 / 11.3 |

### Infrastructure services

| Service | Image | Purpose |
|---|---|---|
| `qdrant` | `qdrant/qdrant:v1.17.0` | Vector ANN (collections `capability_embeddings`, `content_embeddings`, 768-dim cosine) |
| `rustfs` | `quay.io/minio/minio:RELEASE.2025-04-22` (MinIO drop-in for RustFS S3 API) | S3-compatible object storage; default bucket `workspace` |
| `rustfs-init` | (same) | Creates the `workspace` bucket on first start |
| `marker-api` | `ghcr.io/vikparuchuri/marker:latest` (profile `marker`) | PDF → markdown conversion service |
| `agent-gateway` | built from `apps/backend/Dockerfile` | HTTP API + Foundry UI |
| `current-time` | built from `services/current-time/` | Demo self-registering MCP service |
| `web` | `node:22-slim` (profile `web`) | SvelteKit Node build |
| `jaeger` | `jaegertracing/all-in-one:1.58` (profile `observability`) | Trace UI |
| `otel-collector` | `otel/opentelemetry-collector-contrib:0.123` (profile `observability`) | OTLP gRPC/HTTP receiver |

---

## 2. Repository Layout

```
conusai-platform/
├── Cargo.toml                  # Rust workspace (apps/backend/crates/*, evals, src-tauri)
├── package.json                # pnpm workspace + scripts
├── pnpm-workspace.yaml         # apps/* and packages/*
├── pnpm-lock.yaml
├── turbo.json                  # Turborepo task graph
├── biome.json                  # Linter/formatter config
├── playwright.config.ts        # Top-level e2e config (projects: web, ios, shell-macos)
├── docker-compose.yml          # qdrant, rustfs, gateway, current-time, web, jaeger, otel
├── justfile, Makefile, start.sh, stop.sh   # Orchestration helpers
├── rust-toolchain.toml         # stable + wasm32-wasip1
├── renovate.json
│
├── apps/
│   ├── backend/                            # Rust agent gateway monolith
│   │   ├── Dockerfile                      # 4-stage cargo-chef build
│   │   ├── rust-toolchain.toml
│   │   ├── start.sh / stop.sh / start-verify.sh
│   │   ├── scripts/                        # otel-collector.yaml etc.
│   │   ├── capabilities/                   # On-disk capability bundles (TOML + WASM)
│   │   ├── crates/
│   │   │   ├── common/                     # Shared types, error, telemetry, MCP, audit, artifact, trace, memory traits
│   │   │   ├── agent-core/                 # LLM registry, capability registry, semantic router, indexing, stores
│   │   │   ├── jobs/                       # ScheduledJob + BackgroundJob traits, JobExecutor, scheduler
│   │   │   └── agent-gateway/              # Axum HTTP + WS + UI + middleware + routes
│   │   └── evals/                          # Eval CLI (invoice, OCR runners + scorers)
│   │
│   ├── web/                                # SvelteKit public app
│   │   ├── src/
│   │   │   ├── app.html, app.d.ts, hooks.server.ts
│   │   │   ├── lib/
│   │   │   │   ├── sdk.ts                  # Wraps @conusai/sdk for the browser
│   │   │   │   └── server/{env,session}.ts # SSR-only session HMAC + env validation
│   │   │   ├── routes/                     # +page, +layout, +error, login/, logout/
│   │   │   └── tests/
│   │   ├── e2e/smoke.test.ts
│   │   ├── svelte.config.js, vite.config.ts, playwright.config.ts
│   │   └── package.json
│   │
│   └── browser-shell/                      # Tauri 2 cross-platform shell
│       ├── src/                            # SvelteKit (static adapter) UI
│       │   ├── lib/
│       │   │   ├── TraceReplayCapability.svelte   # Renderer registered for the `trace.replay` capability
│       │   │   ├── tauri-stream.ts                # Bridge agent SSE → Svelte store via Tauri events
│       │   │   ├── sdk.ts                         # SDK bound to gateway base URL
│       │   │   └── mobile/                        # MobileShell + responsive variants
│       │   └── routes/                            # +layout, +page
│       └── src-tauri/                      # Tauri Rust core
│           ├── Cargo.toml, build.rs, tauri.conf.json
│           ├── capabilities/{main,ios}-capability.json
│           ├── icons/, macos/, gen/, e2e/
│           └── src/
│               ├── main.rs                 # Entry point → `browser_shell_lib::run()`
│               ├── lib.rs                  # Tauri builder; injects RECORDER_BRIDGE_JS into tabs
│               ├── tabs.rs                 # TabManager — multi-webview tab strip
│               ├── recorder.rs             # SessionRecorder impl; click/input/submit capture; PII redaction
│               ├── chat_stream.rs          # StreamRegistry — forwards backend SSE to webview as Tauri events
│               ├── device_auth.rs          # DeviceAuthService + Stronghold-backed token cache
│               ├── registration.rs         # First-run pairing: POST /admin/devices and persist token
│               └── telemetry.rs
│
├── packages/                               # Shared TS libs (pnpm workspace)
│   ├── types/src/{domain.ts,index.ts}      # SessionTrace, UserStep, WorkspaceNode, CapabilityCard, ControlMessage, FileToken
│   ├── sdk/src/                            # Typed client (auth, capabilities, chat, chatApi, client, endpoints, files, glyphs, realtime, shells, threads, types, ui, workspaces)
│   └── ui/src/lib/
│       ├── tokens.css, foundry.css         # Design tokens + global styles
│       ├── assets/                         # Logo, favicon, fonts
│       ├── stores/                         # featureFlags, modeStore, themeStore, toast
│       ├── utils/                          # LiveAnnouncer, actions, markdown
│       ├── capabilities/                   # CapabilityRendererRegistry (per-capability Svelte renderers)
│       ├── components/                     # AppShell, ArtifactPreview, CapabilityCard, CommandPalette, RecorderControls, TabStrip, ThemeProvider, ThemeScript, ThemeSwitcher, ToastHost, WorkspaceTree
│       └── features/
│           ├── AgentChatComposer.svelte
│           ├── AgentChatStream.svelte
│           ├── ToolCallCard.svelte
│           ├── WorkspaceExplorer.svelte
│           ├── createChatStream.svelte.ts  # Svelte 5 runes-based SSE store
│           ├── auth/LoginPanel.svelte
│           └── workspace/{ConfirmDialog,MoveDialog,NewNodeDialog,ShareDialog}.svelte
│
├── services/
│   └── current-time/                       # Self-registering MCP demo (FastAPI + httpx)
│
├── e2e/                                    # Top-level cross-platform e2e
│   ├── fixtures/seed-workspace.ts
│   ├── helpers/tauri.ts
│   ├── web/                                # Playwright web suite
│   ├── ios/{features,responsive}.spec.ts   # iOS Safari (Playwright)
│   ├── shell-macos/{login,tabs}.spec.ts    # Tauri macOS (Playwright tauri-driver)
│   └── wdio/                               # WebdriverIO: macos/ios/ios-native configs
│
├── workspaces/                             # Tenant workspace mount (indexed by WorkspaceIndexer)
├── docker/                                 # Auxiliary compose overlays / configs
├── scripts/                                # generate-icons.py, generate-logo-variants.py, openapi-to-types.sh, png-to-svg.py
│
└── docs/
    ├── arch.md                             # this document
    ├── plan.md, improve-plan.md, ui-plan.md, ui-design.md
    ├── auth-plan.md
    ├── browser-shell-plan.md, browser-shell-user-guide.md
    ├── capability-authoring-guide.md, capability-gaps-plan.md, capability-gaps-pan.md
    ├── project-instructions.md
    ├── branding/
    ├── ops/signing.md
    ├── tasks/{agent-memory,agent-migration,app,browser-shell-task,coding-agent,
    │          generative-survey-cap,generic-agent,hi-performance-task,job-plan,
    │          local-embeddings,sql-migrations,ui-task}.md
    ├── verify/{verify.md, files/}
    └── adr/
        ├── 0003-unified-postgres-cocoindex.md
        ├── 0003-unified-postgres-vector-search.md
        ├── 0004-semantic-capability-router-and-dynamic-prompts.md
        ├── 006-tauri-browser-shell.md
        ├── 007-capability-module-rename.md
        └── 008-multi-platform-shell.md
```

---

## 3. High-Level Architecture

```
                             ┌──────────────────────────────────────────────────────────┐
                             │                      Clients                             │
                             │                                                          │
                             │  apps/web (SvelteKit + Node SSR)                         │
                             │  apps/browser-shell (Tauri: macOS, Win, Linux, iOS, And) │
                             │  Foundry UI (Askama, served by gateway)                  │
                             │  External API clients (JWT or X-API-Key)                 │
                             └──────────────────────────┬───────────────────────────────┘
                                                        │  HTTPS / WSS
                                                        ▼
            ┌──────────────────────────────────────────────────────────────────────────┐
            │  agent-gateway  (Axum 0.8)                                               │
            │                                                                          │
            │  Middleware stack:                                                       │
            │   TraceLayer → request_id → propagate_trace → api_key → tenant → plan    │
            │                                                                          │
            │  Routers: public · /metrics · protected · admin (super-admin) · ui       │
            │                                                                          │
            │  Per-turn agent loop:                                                    │
            │   ConversationService.resolve_for_node → ContextBuilder (6 kB ancestor   │
            │   context) → SemanticCapabilityRouter.select (Qdrant ANN, top-K) →       │
            │   RouterQuotaLayer enforces budget → CapabilityProvider.invoke           │
            │   → ArtifactBridge materialises files → WorkspaceStore.index_content     │
            └─────────────────┬─────────────────────────────────────────────┬──────────┘
                              │                                             │
                              │ Capabilities (dynamic + on-disk + DB)       │ Storage
                              ▼                                             ▼
   ┌──────────────────────────────────────────────┐   ┌─────────────────────────────────────┐
   │  CapabilityRegistry (Arc<Mutex>)             │   │  RedbMetadataStore (single .redb)   │
   │                                              │   │   - threads / messages              │
   │  Factories:                                  │   │   - workspace_nodes + idx_path      │
   │   - McpFactory          (HTTP JSON-RPC 2.0)  │   │   - audit_events                    │
   │   - WasmFactory         (wasmtime 44)        │   │                                     │
   │   - ChainFactory(llm)   (PromptChain + Rig)  │   │  QdrantVectorStore (HTTP/gRPC)      │
   │   - BuiltinFactory      (fs, cargo)          │   │   - capability_embeddings           │
   │   - DynamicPromptFactory (redb-backed)       │   │   - content_embeddings              │
   │   - RemoteMcpFactory    (self-registered)    │   │   - 768-dim cosine                  │
   │   - TraceReplayFactory  (replay session)     │   │                                     │
   │                                              │   │  RustFsContentStore (object_store)  │
   │  Bulk: CapabilitySpecFactory                 │   │   - tenants/{id}/workspaces/{path}  │
   │   - streams capability_specs rows            │   │                                     │
   │   - hot-reload via redb broadcast            │   │  Files: object_store::AmazonS3      │
   │                                              │   │   - tenants/{id}/{uuid}/{name}      │
   │  Built-in: WorkspaceProvider                 │   │                                     │
   │   (workspace__save_document / list_folders)  │   │  HttpMarkerClient (PDF → markdown)  │
   └──────────────────────────────────────────────┘   └─────────────────────────────────────┘
                              │                                             ▲
                              ▼                                             │
   ┌──────────────────────────────────────────────┐                         │
   │  LlmRegistry (single source of truth)        │                         │
   │   Providers: AnthropicProvider (Rig)         │                         │
   │   4-step alias resolution:                   │                         │
   │     1. tenant.preferred_model                │                         │
   │     2. caller-supplied alias / model         │                         │
   │     3. plan.default_alias()                  │                         │
   │     4. registry default binding              │                         │
   └──────────────────────────────────────────────┘                         │
                                                                            │
   ┌──────────────────────────────────────────────┐                         │
   │  Jobs (cron + on-demand)                     │                         │
   │   ScheduledJob:                              │                         │
   │     - CapabilityHealthCheckJob (*/5 min)     │                         │
   │     - AuditLogCleanupJob       (02:00)       │                         │
   │   BackgroundJob:                             │                         │
   │     - VideoTranscriptionJob (Whisper API)    │ ───── enqueues ─────────┘
   │   JobExecutor — in-memory tracker + SSE      │
   └──────────────────────────────────────────────┘
```

---

## 4. Backend Crates

The Rust workspace is rooted at the **monorepo root** (`/Cargo.toml`) and contains: `apps/backend/crates/{common,agent-core,jobs,agent-gateway}`, `apps/backend/evals`, and `apps/browser-shell/src-tauri`.

### 4.1 `crates/common` — Shared Utilities

**Modules** (`src/`):

| File | Purpose |
|---|---|
| `lib.rs` | Re-exports modules + `prelude` (`Result`, `ConusAiError`, tracing macros) |
| `types.rs` | Typed ID newtypes — `ThreadId`/`NodeId` (ULID-backed), `TenantId`/`UserId`/`ToolId` (string-backed). All `serde(transparent)` |
| `error.rs` | `ConusAiError` (`Config`, `Tool`, `Wasm`, `WasmRuntime`, `Mcp`, `Rig`, `Storage`, `Validation`, `NotFound`, `Api{status,message}`, `Io`, `Other`). `ErrorEnvelope` + `ApiErrorBody` + `ApiErrorKind` (`Authentication`, `RateLimit{retry_after}`, `NotFound`, `Validation{field}`, `Agent`, `Internal{request_id}`). `HttpError` builder + `IntoResponse`. All `ToSchema` for OpenAPI |
| `config/mod.rs` | `AppConfig { server, capabilities_dir, telemetry, llm }`. `LlmConfig { default, aliases, providers }`. Figment TOML + env (`CONUSAI_` prefix) |
| `telemetry.rs` | `init(service, log_level) -> (TelemetryGuard, prometheus::Registry)`. JSON tracing-subscriber + OTLP traces + single `SdkMeterProvider` (Prometheus + OTLP `PeriodicReader`) |
| `http_client.rs` | `build_client()` — `reqwest` with 60 s timeout, UA `conusai-platform/0.1` |
| `mcp.rs` | `JsonRpcRequest` / `JsonRpcResponse` / `JsonRpcError` (JSON-RPC 2.0) |
| `wasm.rs` | `WasmLoader` wrapping `wasmtime::Engine` |
| `limits.rs` | Hardcoded caps: `MAX_PROMPT_TOKENS=128k`, `MAX_RESPONSE_TOKENS=16k`, `MAX_CAPABILITY_SIZE_BYTES=50 MB`, `MAX_WASM_SIZE_BYTES=10 MB`, `REQUEST_TIMEOUT_SECS=120`, `MAX_CONCURRENT_AGENTS=64`, `MAX_MESSAGES_PER_THREAD=10_000`, `MAX_MESSAGES_BEFORE_SUMMARY=50` |
| `path_safety.rs` | `safe_join(root, rel)` — rejects `..`; `join_under_tenant(root, tenant_id, rel)` |
| `audit.rs` | `AuditEvent { id (ULID), tenant_id, timestamp, action, tool?, status, duration_ms?, metadata }`; `AuditStore` async trait (`append`, `list(tenant, limit)`) |
| `metrics.rs` | OTel meters `conusai.agent` (`tool_invocations`, `tool_errors`, `tool_duration_ms`, `llm_requests`, `llm_input_tokens`, `llm_output_tokens`, semantic router + GenAI counters/histograms) and `conusai.storage` (`storage_duration_ms`, `storage_errors`) |
| `eval.rs` | Eval framework shared types |
| `artifact.rs` | **`Artifact { name, mime_type, data?, source_url?, metadata }`** and **`ToolOutput { content, artifacts, metadata }`** — canonical tool result envelope materialised by `ArtifactBridge` |
| `trace.rs` | **Browser-shell session capture types**: `StepKind` (Click/Input/Submit/Navigate/Scroll), `UserStep`, `SessionTrace`, `SessionRecorder` trait (Tauri/headless/mobile), `TraceSource` trait (load JSON from any backing store) |
| `prompt/template.rs` | `PromptTemplate` — `{{key.subkey}}` mustache-like interpolation over `serde_json::Value`. No external template engine |
| `memory/thread.rs` | `Thread { id, tenant_id, title, created_at, last_active, message_count, summary, metadata }`; `Message { role, content, tool_calls, timestamp, seq }`; `ToolCall { id, name, input, output }` |
| `memory/workspace.rs` | `NodeKind { Folder, Conversation, File }`; `WorkspaceNode { id, tenant_id, owner_id, parent_id, kind, name, virtual_path, last_modified, shared_with, metadata }`; helpers `new_folder`, `new_conversation`, `validate_name`, `join_virtual_path`, `effective_user_id` |
| `memory/store.rs` | `ThreadStore`, `WorkspaceStore`, `WorkspaceContentStore` async traits — all take `tenant_id` + `user_id`. ACL: `tenant_id = X AND (owner_id = U OR shared_with @> [U])` |
| `memory/inmem.rs` | `InMemoryThreadStore`, `InMemoryWorkspaceStore`, `InMemoryWorkspaceContent`, `InMemoryAuditStore` — zero-dependency `Mutex<HashMap<…>>`; activated by `CONUSAI_TEST_MODE=1` |
| `memory/tests.rs` | Lib tests covering serde + ACL + validation |

### 4.2 `crates/agent-core` — Agent Runtime

**Top-level modules** (`src/`): `agent`, `bridge`, `capabilities`, `chains`, `context`, `indexing`, `llm`, `memory`, `prompt`, `realtime`, `store`, `vector_store`.

#### LLM abstraction (`src/llm/`)

Single source of truth for all model access — no route, chain, or hook constructs a provider client directly.

| File | Purpose |
|---|---|
| `types.rs` | `LlmRequest` (`bon` builder: `model`, `messages: Vec<rig::Message>`, `temperature`, `max_tokens`, `tools: Vec<Value>`, `tenant`), `LlmResponse { content, usage, finish_reason }`, `LlmUsage`, `LlmChunk`, `LlmStream` (`Pin<Box<dyn Stream<Item=Result<LlmChunk,LlmError>> + Send>>`), `LlmBinding { provider, model }` |
| `error.rs` | `LlmError`: `Config`, `Request`, `Response`, `UnknownAlias`, `ProviderNotFound`, `Streaming` |
| `provider.rs` | `CompletionProvider` async trait: `name`, `supports_tools`, `supports_vision`, `supports_streaming`, `complete`, `stream` |
| `registry.rs` | `LlmRegistry { providers, aliases, default }`; 4-step `resolve_binding` (tenant → caller → plan → default); `verify_llm_providers` validates at boot |
| `streaming.rs` | OpenAI-compatible SSE helpers |
| `providers/anthropic.rs` | `AnthropicProvider` wrapping `rig::providers::anthropic::Client`; `from_env()` reads `ANTHROPIC_API_KEY`; streams via Rig 0.36 native SSE (`CompletionModel::stream` → `StreamedAssistantContent::Text` → `LlmChunk`); `supports_vision = true` |

#### Agent subsystem (`src/agent/`)

| File | Purpose |
|---|---|
| `builder.rs` | `Agent`, `AgentBuilder` (`model`, `preamble`, `max_tokens`, `with_tenant`, `build`). Enforces `plan.max_tokens()`. `prompt(text)` attaches `TracingHook` and `plan.max_turns()` |
| `hooks.rs` | `TracingHook { tenant_id, plan, thread_id }` implements `rig::agent::PromptHook<M>` (`on_completion_call`, `on_tool_call`). `PermissionHook` for future ACL checks |
| `runtime.rs` | `AgentRuntime`; `map_rig_error(msg)` pattern-matches Rig error strings to typed `HttpError` |

#### Context subsystem (`src/context/`)

| File | Purpose |
|---|---|
| `tenant.rs` | `UserRole { User, Admin, SuperAdmin }`, `PlanTier { Free, Pro, Enterprise }` with `max_tokens()` (4 k / 16 k / 128 k), `max_turns()` (3 / 8 / 20), `rate_limit_rpm()` (10 / 60 / 600), `default_alias()` (`haiku` / `opus` / `opus`). `TenantContext { tenant_id, user_id, plan, role, workspace_root, preferred_model }` with `tenant_root`, `safe_path`, `storage_prefix`, `system_prompt`, `span_fields`. `TenantClaims { sub, tenant_id, plan, role, exp }` |
| `conversation.rs` | `ConversationService` trait: `create`, `append_message`, `load_history`, `resolve_for_node` (lazy bind), `list`, `get`. `DefaultConversationService { thread_store, workspace_store }` |
| `mod.rs` | `ConversationContext` — message history → `rig::Message` |

#### Capabilities subsystem (`src/capabilities/`)

| File | Purpose |
|---|---|
| `manifest.rs` | `ToolManifest { name, version, description, kind, tools, config, tags, namespace, chain, tenant_scope, enabled, search_keywords }`; `ToolKind { Mcp, Wasm, Chain, Docker, Native, DynamicPrompt, RemoteMcp }`; `ToolDef`; `LlmChainConfig { model, system_prompt?, prompt_template, vision, max_tokens, output_schema? }`; `from_toml`, `from_file`, `embedding_text` |
| `card.rs` | `CapabilityCard { id, manifest, source_dir, embedding_id?, enabled, last_error?, registered_at, updated_at, provider }` |
| `provider.rs` | `CapabilityProvider` async trait (`manifest`, `invoke`, `tool_definitions`, `invoke_typed`); `CapabilityFactory` trait (`supports`, `create`); `BulkCapabilityFactory` trait |
| `registry.rs` | `CapabilityRegistry { cards, factories, bulk_factories, namespace_index }`. `with_default_factories(llm)` registers Mcp/Wasm/Chain/Builtin; `with_all_factories(llm)` also adds `DynamicPromptFactory`, `TraceReplayFactory`, `RemoteMcpFactory`. Mutators: `register`, `unregister`, `replace`, `set_enabled`, `reload_capability`, `register_bulk_factory`, `run_bulk_load`. Queries: `get`, `get_provider`, `all`, `all_enabled`, `search_by_tag`, `namespace_children(prefix)` |
| `discovery.rs` | `CapabilityDiscovery::from_env()` reads `CONUSAI_CAPABILITIES_DIR` (default `./capabilities`); `discover_into(&mut registry)` |
| `store.rs` | `RegisteredToolState`, `RegisteredToolStore` trait; `FilesystemStore` (atomic `.tmp`→ rename writes) |
| `validator.rs` | `RegisteredToolValidator` (regex `^[a-z0-9-]{2,64}$`, manifest + WASM size), `ValidationReport` |
| `admin.rs` | `CapabilityAdmin` — coordinates `FilesystemStore` + registry + validator + audit store. `AdminLimits` env-overridable. `build_admin()` factory |
| `executor.rs` | `ToolExecutor::invoke(registry, cap, tool, input, tenant)` — `#[instrument]`; emits `tool_invocations`, `tool_duration_ms`, `tool_errors` metrics |
| `mcp_adapter.rs` | `McpAdapter` — JSON-RPC 2.0 HTTP client (`call`, `list_tools`, `call_tool`) |
| `wasm_loader.rs` | `WasmToolLoader` wrapping wasmtime 44 (Component Model) |
| **`namespace.rs`** | `NamespaceFilter { Any, Exact, Prefix, AnyOf }` + dot-segment validator (`[a-z][a-z0-9_]*`, ≤ 6 segments) |
| **`embedding.rs`** | Helpers to chunk manifest text and persist via `EmbeddingService` + `QdrantVectorStore` |
| **`semantic_router.rs`** | `SemanticCapabilityRouter` — blake3 query-cache (moka, default 60 s TTL) → embed → Qdrant ANN top-K → namespace + tag filter → distance threshold (≤ 0.65) → `include_always` overrides. Returns ≤ `top_k` `Arc<dyn CapabilityProvider>`. `SemanticRouterConfig { top_k, max_distance, namespace, tags_any, include_always, cache_ttl_secs }`. `RouterMetrics` (atomic counters) |
| **`trace_replay.rs`** | `TraceReplayCapability` — replays a `SessionTrace` against an LLM to produce a dry-run plan. `TraceReplayFactory`. `WorkspaceNodeTraceSource` loads JSON from a workspace conversation |
| `providers/mcp.rs` | `McpProvider` + `McpFactory` |
| `providers/wasm.rs` | `WasmProvider` + `WasmFactory` |
| `providers/chain.rs` | Hardcoded chain adapters (`InvoiceProvider`, `ContractProvider`, `OcrProvider`) plus the data-driven `PromptChainCapability` path; `ChainFactory::new(llm)` |
| `providers/builtin.rs` | `BuiltinProvider` + `BuiltinFactory` — routes to `builtin/{fs,cargo}` |
| `providers/dynamic_prompt.rs` | `DynamicPromptCapability` + `DynamicPromptFactory` — loads versioned `LlmChainConfig` from the `dynamic_prompts` redb table; `load_latest`, `with_pinned_version(n)`, `invalidate`. 60 s moka cache |
| `providers/remote_mcp.rs` | `RemoteMcpCapability` + `RemoteMcpFactory` — self-registered external MCP services |
| `providers/capability_spec.rs` | `CapabilitySpecFactory` (`BulkCapabilityFactory`) — streams rows from `capability_specs` in 256-row chunks, batch-embeds, upserts to Qdrant, and registers `CapabilityProvider` instances. Hot-reload via in-process broadcast from `RedbMetadataStore` |
| `builtin/fs.rs` | `read_file` / `write_file` — tenant-scoped `safe_join`; `tokio::fs` |
| `builtin/cargo.rs` | `run_cargo` — allowlisted subcommands (`check`, `test`, `build`, `clippy`, `fmt`) via `tokio::process::Command` |
| `builtin/card.rs` | `builtin_tool_card()` — `CapabilityCard` (`kind = Native`) with full JSON schemas |

#### Chains (`src/chains/`)

| File | Purpose |
|---|---|
| `extraction.rs` | `ExtractionPipeline` async trait — base64 + Claude vision + strict JSON schema; default `extract_from_bytes`, `extract_as_value` |
| `invoice.rs` | `InvoicePipeline`, `InvoiceData`, `InvoiceLineItem` (~ 20 fields, `JsonSchema`); default `claude-opus-4-7`; png/jpeg/jpg/pdf |
| `contract.rs` | `ContractPipeline`, `ContractData`, `ContractParty` |
| `llm_chain.rs` | `PromptChainCapability { manifest, cfg: LlmChainConfig, prompt: PromptTemplate, llm: Arc<LlmRegistry> }` — renders `{{input.*}}` / `{{tenant.*}}`, calls `LlmRegistry::resolve` + `CompletionProvider::complete`, optional `output_schema` validation. Zero-code TOML capabilities |
| `dynamic_prompt.rs` | Helpers wiring DB rows into `PromptChainCapability` at runtime |
| `executor.rs` | Chain dispatcher shared between the hardcoded pipelines |

#### Prompt (`src/prompt/mod.rs`)

Re-exports `common::prompt::PromptTemplate` (lightweight `{{key.subkey}}` mustache over `serde_json::Value`; missing paths → empty string).

#### Memory (`src/memory/`)

| File | Purpose |
|---|---|
| `mod.rs` | Re-exports |
| `context_builder.rs` | `ContextBuilder { store, content, truncator }`. `build_for_node(tenant, node_id, max_chars)` — walks ancestors, loads `CONTEXT.md` / `README.md` from object store per folder, loads conversation body, joins with `\n\n---\n\n`, delegates to `ContextTruncator`. Prefixes `# Workspace context\n`. Used by `routes/agent.rs` with `max_chars = 6000` |
| `truncator.rs` | `ContextTruncator` strategy trait; `OldestFirstTruncator` (default) — drops oldest sections, preserves last |

#### Indexing (`src/indexing/`)

| File | Purpose |
|---|---|
| `coco_indexer.rs` | `WorkspaceIndexer` — crawls `WORKSPACES_ROOT`, chunks content, generates embeddings, upserts into Qdrant `content_embeddings` |
| `embedding_service.rs` | `EmbeddingService` trait; `OpenAiEmbeddingService` (default, `text-embedding-3-small`, 1536 dims — collapsed/projected to 768-dim Qdrant); `NoopEmbeddingService` |
| `local_embedding_service.rs` | `LocalEmbeddingService` (feature `local-embeddings`) using `fastembed` 5 — on-device, defaults to `nomic-embed-text` (768 dims), cached in `apps/backend/.fastembed_cache/` |
| `real_fs_watcher.rs` | `RealFsWatcher::spawn(indexer)` — polls filesystem for changes and triggers incremental re-indexing |

#### Realtime (`src/realtime/mod.rs`)

`RealtimeService` — multi-channel tokio broadcast:
- `WorkspaceChangeEvent` fanned out to WS subscribers (`GET /api/realtime/workspace`).
- `subscribe_capability_spec_changes() -> mpsc::Receiver<(namespace, tool_name)>` — fed by `RedbMetadataStore`'s in-process broadcast; consumed by `main.rs` which calls `CapabilitySpecFactory::reload_one(...)`.

#### Bridge (`src/bridge/`)

`ArtifactBridge { file_store, workspace_content }` — after each tool invocation that returns a `ToolOutput`, uploads each `Artifact` to RustFS, inserts a `workspace_nodes` row (`kind = File`), and best-effort enqueues indexing for text-like MIME types.

#### Store (`src/store/`) — **persistent backends**

| File | Purpose |
|---|---|
| `redb_metadata.rs` | **`RedbMetadataStore`** — single embedded redb database implementing `ThreadStore` + `WorkspaceStore` + `AuditStore`. Tables (all postcard-serialised): `threads (tenant_id, thread_id)`, `messages (tenant_id, thread_id, seq)`, `workspace_nodes (tenant_id, node_id)`, `idx_nodes_by_path (tenant_id, virtual_path) → node_id`, `audit_events (tenant_id, ts_micros, event_id)`. Single write transaction per mutation; non-blocking snapshot reads. In-process broadcast channel for capability-spec changes. Constructors: `open(path)`, `in_memory()` |
| `qdrant_vector.rs` | **`QdrantVectorStore`** — Qdrant gRPC client. Collections `capability_embeddings` and `content_embeddings`, both 768-dim cosine. Bootstraps collections on connect. `CapabilityHit` + `ContentHit` DTOs match the old `PgVectorStore` interface so callers (semantic router, indexer) need only a type swap. `connect(url)` for production, `noop()` for test mode |
| `rustfs_content.rs` | **`RustFsContentStore`** — implements `WorkspaceContentStore` via `object_store::AmazonS3`. Keys `tenants/{tenant_id}/workspaces/{virtual_path}`. Env: `S3_ENDPOINT`, `S3_BUCKET`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`. Falls back to `NoopWorkspaceContent` when not configured |
| `marker.rs` | **`MarkerClient`** trait + `HttpMarkerClient` (`reqwest` multipart POST to `{MARKER_URL}/convert`) + `NoopMarkerClient` — used by capabilities that need PDF → markdown |

#### Vector store façade (`src/vector_store/mod.rs`)

Re-exports `QdrantVectorStore` as the canonical vector backend.

**Public `lib.rs` re-exports:** `Agent`, `AgentBuilder`, `TracingHook`, `PermissionHook`, `map_rig_error`, `ArtifactBridge`, capability admin/CRUD types, `CapabilityDiscovery`, `CapabilityFactory`, `BulkCapabilityFactory`, `CapabilityRegistry`, `CapabilitySpecFactory`, `SemanticCapabilityRouter` + config + metrics, `NamespaceFilter`, `TraceReplayCapability`/`Factory`/`WorkspaceNodeTraceSource`, `RegisteredToolStore`/`Validator`, contract/invoice/OCR pipelines, `PromptChainCapability`, `ConversationService`/`Default…`, `PlanTier`/`TenantClaims`/`TenantContext`/`UserRole`, `ContextBuilder`/`ContextTruncator`/`OldestFirstTruncator`, `EmbeddingService`/`Noop`/`OpenAi`/`LocalEmbeddingService` (feature-gated)/`WorkspaceIndexer`, `RealtimeService`/`WorkspaceChangeEvent`, **`HttpMarkerClient`/`MarkerClient`/`NoopMarkerClient`/`QdrantVectorStore`/`RedbMetadataStore`/`RustFsContentStore`**, LLM types.

### 4.3 `crates/jobs`

**Dependency design:** depends only on `common` (never on `agent-core`) — gateway-owned capabilities that need both (e.g. `TranscribeVideoCapability`) live in `agent-gateway`.

| File | Purpose |
|---|---|
| `job.rs` | `TaskState { Queued, Running, Completed, Failed }`, `TaskStatus`, `TaskEvent`, `ScheduledJob` (`name`, `cron`, `enabled`, `run(ctx)`), `BackgroundJob` (`name`, `run(input, ctx)`) |
| `context.rs` | `JobContext { audit_store, minio_endpoint?, bucket? }` — `Arc`-cheap clone |
| `registry.rs` | `JobRegistry { scheduled, background, ctx }`; `register_scheduled`, `register_background` |
| `scheduler.rs` | `JobSchedulerService::start(registry)` — uses `tokio-cron-scheduler` 0.13 to spawn each enabled `ScheduledJob` |
| `executor.rs` | `JobExecutor { tasks: RwLock<HashMap<Uuid, TaskStatus>>, channels: RwLock<HashMap<Uuid, Sender<TaskEvent>>> }` — `enqueue` → `task_id`, `get_status`, `list_tasks`, `subscribe` (SSE) |
| `admin.rs` | `JobAdmin { registry, executor }` — `list_jobs`, `get_job`, `run_now`, `list_tasks`, `get_task` |
| `jobs/capability_health_check.rs` | Cron `0 */5 * * * *` — probes MinIO/RustFS `/minio/health/live` |
| `jobs/audit_log_cleanup.rs` | Cron `0 0 2 * * *` — reads `AUDIT_RETENTION_DAYS` (default 30) |
| `jobs/video_transcription.rs` | `VideoTranscriptionJob` — downloads from RustFS, calls OpenAI Whisper (`OPENAI_API_KEY`) or returns placeholder; output `{ file_id, tenant_id, transcript, chars }` |

### 4.4 `crates/agent-gateway`

#### Entry point (`src/main.rs`)

1. `common::telemetry::init("agent-gateway", "info")` → JSON logs + OTLP + Prometheus registry.
2. `AppState::from_env()` — see below.
3. Spawn capability-spec hot-reload listener (subscribes to `RealtimeService`).
4. Register the runtime-only `TranscribeVideoCapability` (needs `Arc<JobExecutor>`).
5. `verify_llm_providers` (warn-only).
6. `JobSchedulerService::start` (cron loop).
7. If `WORKSPACES_ROOT` is set, spawn an initial `WorkspaceIndexer::index_once` + `RealFsWatcher::spawn`.
8. Assemble router: `public_router` ∪ `/metrics` ∪ `protected_router` (with full middleware stack) ∪ `ui_router` ∪ `admin_router` ∪ static `/assets` (`ServeDir`).
9. Apply outer layers: `build_cors()` → `TraceLayer::new_for_http()`.
10. `axum::serve` on `0.0.0.0:8080`.

#### `AppState` (`src/state.rs`)

```rust
pub struct AppState {
    pub registry: Arc<Mutex<CapabilityRegistry>>,
    pub rate_limiter: RateLimiter,
    pub llm: Arc<LlmRegistry>,
    pub file_store: Option<Arc<dyn ObjectStore>>,            // RustFS / S3 (uploads)
    pub presigned_tokens: Mutex<HashMap<String, (String, Instant, Duration, String)>>,
    pub device_tokens: Mutex<HashMap<String, DeviceToken>>,  // blake3(token) → record
    pub thread_store: Arc<dyn ThreadStore>,                  // RedbMetadataStore
    pub audit_store: Arc<dyn AuditStore>,                    // RedbMetadataStore
    pub workspace_store: Arc<dyn WorkspaceStore>,            // RedbMetadataStore
    pub workspace_content: Arc<dyn WorkspaceContentStore>,   // RustFsContentStore | Noop
    pub conversation_service: Arc<dyn ConversationService>,
    pub tool_admin: Arc<CapabilityAdmin>,
    pub job_registry: Arc<JobRegistry>,
    pub job_executor: Arc<JobExecutor>,
    pub job_admin: Arc<JobAdmin>,
    pub embedding_service: Arc<dyn EmbeddingService>,
    pub vector_store: Arc<QdrantVectorStore>,
    pub realtime_service: Arc<RealtimeService>,
    pub semantic_router: Arc<SemanticCapabilityRouter>,
    pub router_quota: RouterQuotaConfig,
    pub capability_spec_factory: Option<Arc<CapabilitySpecFactory>>,
    pub artifact_bridge: Option<Arc<ArtifactBridge>>,
}
```

`from_env()` boot order:

1. `CONUSAI_TEST_MODE=1` → `with_in_memory_stores()` (no external services).
2. Warn if `PLATFORM_ADMIN_TOKEN` is unset in non-debug builds.
3. `LlmRegistry` (Anthropic provider; default binding `anthropic / claude-haiku-4-5`).
4. **`RedbMetadataStore::open(REDB_PATH)`** (default `/data/conusai.redb`) — shared across `ThreadStore`/`WorkspaceStore`/`AuditStore`.
5. **`QdrantVectorStore::connect(QDRANT_URL)`** (default `http://qdrant:6334`).
6. **`RustFsContentStore::from_env()`** for workspace content; `init_file_store()` for general file uploads (both default to `http://rustfs:9000`, bucket `workspace`, credentials `minioadmin/minioadmin` for dev).
7. `EmbeddingService` from `EMBEDDING_BACKEND` (`local` → fastembed if feature compiled, else noop; `openai` or unset → OpenAI; anything else → config error).
8. `CapabilityRegistry::with_all_factories(llm)` → `CapabilityDiscovery::from_env().discover_into(...)` → `CapabilitySpecFactory::load_batch(...)` → register `WorkspaceProvider`.
9. `SemanticCapabilityRouter` (env `SEMANTIC_ROUTER_TOP_K` default 20; `include_always = ["workspace"]`).
10. `DefaultConversationService`, `CapabilityAdmin`.
11. `JobContext` + `build_job_registry` (pre-registers `CapabilityHealthCheckJob`, `AuditLogCleanupJob`, `VideoTranscriptionJob`) + `JobExecutor` + `JobAdmin`.
12. `ArtifactBridge` (only if `file_store` is `Some`).
13. `RealtimeService::new()` and `RouterQuotaConfig::from_env()`.

#### Auth subsystem (`src/auth/`)

| File | Purpose |
|---|---|
| `verifier.rs` | `SessionUser { name, plan, role, exp }`, `sign(user)`/`verify(token)` HMAC-SHA256 (`UI_SESSION_KEY`). `SESSION_HEADER = "x-session-token"`. `COOKIE_NAME = "conusai_session"`. TTL 86 400 s |
| `extractor.rs` | `extract_from_headers(headers)` — tries `conusai_session` cookie, then `X-Session-Token` header (used by Tauri WKWebView which cannot send Secure cookies cross-origin). Axum `FromRequestParts for SessionUser` returns 401 if neither |
| `mod.rs` | Re-exports |

#### Middleware (`src/mw/`)

| File | Purpose |
|---|---|
| `request_id.rs` | `inject_request_id` — reads `X-Request-ID` or generates UUID; echoes header; rewrites JSON 4xx/5xx error bodies (up to 1 MiB) with `error.request_id` |
| `trace.rs` | `propagate_trace` — extracts W3C `traceparent`/`tracestate` via `TraceContextPropagator` |
| `api_key.rs` | `extract_api_key` — reads `X-API-Key`; BLAKE3-hashes; validates against `API_KEYS` env (`<blake3_hex>:<tenant_id>:<plan>` CSV); sets `ResolvedTenant`; rejects 401 if header present but invalid |
| `tenant.rs` | `extract_tenant` — skips if `ResolvedTenant` already set; production (`JWT_SECRET` set): HS256 Bearer JWT or session cookie/header (via `auth::extractor`); dev: `X-Tenant-ID` or `dev` default + Enterprise plan |
| `plan.rs` | `enforce_plan` — validates `PlanTier` after auth |
| `admin.rs` | `require_super_admin_jwt` / `require_super_admin_session` for `/admin/*` and `/super-admin/*` |
| `rate_limit.rs` | `RateLimiter` — per-tenant 60 s sliding window; plan-based RPM caps |
| **`router_quota.rs`** | `RouterQuotaConfig { max_tools_per_turn (env `CONUSAI_MAX_TOOLS_PER_TURN`, default 25), max_invokes_per_turn (env `CONUSAI_MAX_INVOKES_PER_TURN`, default 10) }` + `RouterQuotaLayer` Tower layer applied to `/v1/agent/completions` |

#### Routes (`src/routes/`)

`routes::mod` assembles four sub-routers:

- **`public_router()`** — no auth: `GET /health`, `POST /v1/auth/login`, `GET /v1/files/{token}` (UUID presigned), `POST /admin/capabilities/register` (gated by `PLATFORM_ADMIN_TOKEN` when set), Swagger UI at `/docs` + `/openapi.json`.
- **`protected_router()`** — full middleware stack:
  - `POST /v1/chat/completions` (`chat.rs`) — OpenAI-compatible chat (blocking + SSE).
  - `POST /v1/agent/completions` (`agent.rs`) — thread-aware tool-calling loop. Wrapped by `RouterQuotaLayer`. Resolves thread (explicit → workspace node metadata → lazy bind), loads history + summary, runs `ContextBuilder` (6000 chars), executes ≤ `plan.max_turns` Anthropic `tool_use` rounds, calls `SemanticCapabilityRouter.select` + `ToolExecutor.invoke`, materialises artifacts via `ArtifactBridge`, indexes last 30 messages into Qdrant. SSE emits OpenAI chunks plus `tool_call_start` / `tool_call_result` events. Span attributes include `gen_ai.*`.
  - `GET /v1/capabilities` (`capabilities.rs`).
  - `GET /v1/capabilities/search?q=&limit=` (`search.rs`) — Qdrant ANN; falls back to local substring match on failure; max limit 20.
  - `POST /mcp` (`mcp.rs`) — JSON-RPC 2.0 dispatcher (`initialize`, `tools/list`, `tools/call` with `capability__tool` slug split).
  - `POST /v1/files` (`files.rs`) — multipart → RustFS at `tenants/{tenant_id}/{uuid}/{filename}`; returns 1 h TTL UUID download token.
  - `GET /v1/audit?limit=` (`audit.rs`) — newest-first, max 500.
  - `POST/GET/PATCH/DELETE /v1/workspaces/...` (`workspaces.rs`) — `create`, `tree`, `search`, `get_node`, `delete_node`, `get_content`, `patch_content`, `move`, `share`, `unshare`.
  - `GET /v1/tasks`, `GET /v1/tasks/{id}`, `GET /v1/tasks/{id}/sse` (`tasks.rs`) — task polling + SSE.
  - `GET /v1/threads/{id}/messages` (`threads.rs`).
  - `GET /api/realtime/workspace` (`realtime.rs`) — WS subscriber on `RealtimeService`.
  - `GET /v1/shells/{device_id}/control` (`shells.rs`) — browser-shell WS control channel (gated by `CONUSAI_FEATURE_BROWSER_SHELL=1`; validates device token in query; sends `Heartbeat`/`Replay`/`Stop`/`Ack` `ControlMessage`s; enforces `CONUSAI_MAX_REPLAYS_PER_TURN`, default 3).
- **`admin_router()`** — `require_super_admin_jwt`:
  - Capability CRUD: list, create, reload all, validate, test, get one, get manifest, update, set enabled, delete, reload one.
  - Dynamic prompt: `PUT /admin/capabilities/{name}/prompt`, `GET /admin/capabilities/{name}/prompt[?version=N]`, `GET /admin/capabilities/{name}/prompt/versions`.
  - Namespace browser: `GET /admin/capabilities/namespaces?prefix=`.
  - Jobs: `GET /admin/jobs`, `GET /admin/jobs/{name}`, `POST /admin/jobs/{name}/run`, `GET /admin/tasks`.
  - Devices (`admin_devices.rs`): `POST /admin/devices` (issue browser-shell pairing token; returns plaintext once, stores blake3 hash), `GET /admin/devices`, `DELETE /admin/devices/{id}`.
- **`ui_router()`** — Foundry server-rendered UI (see below).

OpenAPI: `ApiDoc` registers `bearer_auth` (HS256 JWT), `api_key_auth` (`X-API-Key`), and `cookie_auth` (`conusai_session`) security schemes. Tags: `auth`, `chat`, `agent`, `capabilities`, `mcp`, `workspaces`, `audit`, `files`, `admin`.

#### Built-in capabilities owned by the gateway (`src/capabilities/`)

| File | Purpose |
|---|---|
| `workspace.rs` | **`WorkspaceProvider`** — registered with `include_always = ["workspace"]` so it survives semantic router pruning. Tools: `workspace__save_document(folder_name, filename, content)` (creates folder if missing; appends `.md`) and `workspace__list_folders()`. Backed by `WorkspaceStore` + `WorkspaceContentStore` |
| `transcribe_video.rs` | `TranscribeVideoCapability` — `transcribe(file_id)` enqueues `VideoTranscriptionJob` via `JobExecutor`, returns `{task_id, status:"queued", poll_url}` |
| `mod.rs` | Re-exports |

#### Foundry UI (`src/ui/`)

| File | Purpose |
|---|---|
| `routes.rs` | `ui_router()` — `/`, `/login`, `/logout`, `/ui/*`, `/super-admin/*` (the latter gated by `require_super_admin_session`) |
| `session.rs` | HMAC-SHA256 signed session cookie (re-exported from `auth::verifier`) |
| `handlers/auth.rs` | `GET /login`, `POST /login`, `GET /logout` |
| `handlers/app.rs` | `GET /` — Askama `app.html` (recent threads, capabilities, workspace tree) |
| `handlers/chat.rs` | `POST /ui/stream` — SSE agent stream via in-process invocation |
| `handlers/upload.rs` | `POST /ui/upload` — multipart → RustFS |
| `handlers/invoice.rs` | `POST /ui/extract-invoice` — token → bytes → `InvoicePipeline::extract_from_bytes` |
| `handlers/files.rs` | UI file download helpers |
| `handlers/mod.rs` | Re-exports |

**Templates** (`templates/`): only the `shared/` partials remain in this snapshot — `app.html` and the super-admin views render via Askama against the shared `head.html`. The full Foundry UI (chat composer + workspace explorer) is also reachable from `apps/web` which renders the same flows in Svelte 5.

**Assets** (`assets/`): `css/style.css` (~1320 lines editorial design system), `js/app.js` (~660 lines streaming + composer), `js/workspace.js` (~750 lines tree + dialogs), `icons/icons.svg` sprite, `images/{favicon,logo-light,logo-dark}.png`.

### 4.5 `evals`

Same shape as before. CLI: `cargo run -p evals -- run --suite <name> --dataset <path?> --model <id>` / `list`.

| Path | Purpose |
|---|---|
| `src/main.rs` | `clap`-based CLI |
| `src/runners/invoice.rs` | Loads JSONL `{image_path, expected}`; runs `InvoicePipeline`; scores via `InvoiceScorer` (case-insensitive string + `abs(diff) < 0.01` numeric; 7-field comparison; `pass_threshold = 0.8`) |
| `src/runners/ocr_quality.rs` | Sends image through `ocr-service` capability via gateway; requires `GATEWAY_URL` |
| `src/scorers/mod.rs` | `ScorerResult`, `InvoiceScorer` |
| `src/report.rs` | Summary table |
| `datasets/{invoice,ocr_quality}.jsonl` | Test corpora |

---

## 5. Capabilities (`apps/backend/capabilities/`)

Drop a folder with `capability.toml` (and optionally `capability.wasm`) — the registry auto-discovers and loads it at startup or on admin reload.

### Capability kinds

| Kind | Runtime | Wire format |
|---|---|---|
| `mcp` | External HTTP/stdio process | JSON-RPC 2.0 (`McpAdapter`) |
| `remote_mcp` | Self-registered external MCP | `RemoteMcpCapability` + `POST /admin/capabilities/register` |
| `wasm` | Wasmtime 44 (`wasm32-wasip1`, Component Model) | Exported WASM functions |
| `chain` (hardcoded) | In-process Rig pipeline | `InvoicePipeline` / `ContractPipeline` / `OcrProvider` |
| `chain` (data-driven) | `PromptChainCapability` via `LlmRegistry` | TOML `[chain]` block + `PromptTemplate` |
| `dynamic_prompt` | `DynamicPromptCapability` | redb `dynamic_prompts` table; versioned prompt rows |
| `native` | In-process Rust | `BuiltinProvider` (fs, cargo) + `WorkspaceProvider` |
| `docker` | Container (reserved) | TBD |

### Data-driven chain example

```toml
kind = "chain"
[chain]
model = "opus"                   # LlmRegistry alias or concrete model id
system_prompt = "You are …"
prompt_template = "{{input.text}}"
vision = false
max_tokens = 2048
output_schema = { /* optional JSON Schema */ }
```

`{{input.*}}` and `{{tenant.id}}` / `{{tenant.plan}}` placeholders are resolved by `PromptTemplate`.

### Bundled capabilities

| Folder | Kind | Tools | Notes |
|---|---|---|---|
| `file-storage/` | mcp | `upload_file`, `download_file`, `presigned_url` | Manifest only — actual storage lives in `routes/files.rs` |
| `google-workspace/` | mcp | `list_files`, `read_document`, `append_to_sheet`, `send_email` | OAuth2 scopes: drive.readonly, documents.readonly, spreadsheets, gmail.send |
| `invoice-processing/` | chain | `extract_invoice`, `validate_invoice` | `InvoicePipeline`; default `claude-opus-4-7`; max 20 MB; png/jpeg/jpg/pdf |
| `contract-processing/` | chain | `extract_contract`, `summarise_contract` | `ContractPipeline` |
| `ocr-service/` | chain | `extract_text` | `OcrProvider`; default `claude-sonnet-4-6` |
| `runtime-echo/` | chain | `echo` | Data-driven minimal chain (`claude-opus-4-7`, 128 max_tokens); for runtime testing |
| `template-wasm/` | wasm | `ping` | Loads `capability.wasm`; exports `ping() -> i32 = 42` |

`trace.replay` (kind `dynamic_prompt`) is registered at runtime by `TraceReplayFactory`; the Browser Shell ships a `TraceReplayCapability.svelte` renderer.

`services/current-time/` is the canonical **self-registering MCP** demo: on container start it POSTs to `/admin/capabilities/register` with its manifest + endpoint URL, then serves JSON-RPC 2.0 tool calls (`get_current_time(timezone?)`) at `:8082`.

---

## 6. Frontend Apps

### 6.1 `apps/web` — SvelteKit (Node adapter)

- **Entry:** `src/app.html`, `src/app.d.ts`, `src/hooks.server.ts`.
- **`hooks.server.ts`** — disables SvelteKit's blanket CSRF origin check on backend-proxied prefixes (`/v1`, `/api`, `/ui`, `/mcp`, `/admin`), enforces a scoped origin check on form paths, and parses the `conusai_session` cookie via `$lib/server/session.ts` → `event.locals.user`.
- **`src/lib/sdk.ts`** — instantiates `createConusSdk({ fetch, baseUrl: '', tokenProvider })`.
- **`src/lib/server/{env,session}.ts`** — production env-var validation; HMAC-SHA256 cookie sign/verify with `UI_SESSION_KEY`.
- **Routes:** `+layout.{svelte,server.ts}`, `+page.{svelte,server.ts}`, `+error.svelte`, `login/`, `logout/`. The root page wires `@conusai/ui` features: `AgentChatStream`, `AgentChatComposer`, `WorkspaceExplorer`, `createChatStream(sdk)`, `provideCapabilityRendererRegistry()`, `ThemeSwitcher`. Selecting a workspace conversation node auto-loads its `metadata.thread_id` into the chat stream.

### 6.2 `apps/browser-shell` — SvelteKit + Tauri 2

#### SvelteKit shell (`src/`)

| Path | Purpose |
|---|---|
| `lib/sdk.ts` | SDK with explicit `baseUrl` (gateway URL) and `X-Session-Token` injection (WKWebView cannot use cookies) |
| `lib/tauri-stream.ts` | Bridges Tauri events (emitted by `chat_stream.rs`) into Svelte stores |
| `lib/TraceReplayCapability.svelte` | Renderer registered for the `trace.replay` capability so replay plans display natively |
| `lib/mobile/` | Mobile-first variants — `MobileShell` is the root for iOS/Android |
| `routes/+layout.svelte`, `routes/+page.svelte` | Sets `modeStore = 'shell'`, registers the trace-replay renderer, mounts `MobileShell` |

#### Tauri Rust core (`src-tauri/`)

| File | Purpose |
|---|---|
| `Cargo.toml`, `build.rs`, `tauri.conf.json` | Tauri 2 manifest. Plugins: `dialog`, `stronghold` |
| `capabilities/main-capability.json`, `ios-capability.json` | Tauri ACL — which commands each window may invoke |
| `icons/`, `macos/`, `gen/` | App icons, macOS Info.plist additions, generated bindings |
| `e2e/` | Tauri-driver Playwright fixtures |
| `src/main.rs` | Calls `browser_shell_lib::run()` |
| `src/lib.rs` | Tauri builder: state (`TabManager`, `RecorderState`, `DeviceAuthService`), command registration (`recorder_record_step`, …), injects `RECORDER_BRIDGE_JS` into every child webview. The bridge captures `click`/`change`/`submit` DOM events and forwards them to the Rust recorder; PII fields (`password|ssn|cc-|card|cvv`) are redacted |
| `src/tabs.rs` | `TabManager` — multi-webview tab strip; one webview per tab |
| `src/recorder.rs` | Implements `common::trace::SessionRecorder`; produces `SessionTrace` JSON consumable by `TraceReplayCapability` |
| `src/chat_stream.rs` | `StreamRegistry` — fans backend SSE chunks out as Tauri events the SvelteKit UI subscribes to |
| `src/device_auth.rs` | `DeviceAuthService` — caches the pairing token in **Tauri Stronghold** (encrypted file). Provides `DeviceTokenProvider` consumed by SDK requests. E2E bypass via env in debug builds |
| `src/registration.rs` | First-run pairing flow: POST `/admin/devices` with `PLATFORM_ADMIN_TOKEN`, persist returned plaintext token |
| `src/telemetry.rs` | Local tracing init |

### 6.3 Shared `packages/`

| Package | Public API |
|---|---|
| `@conusai/types` | `ToolKind`, `CapabilityCard`, `UserStep`, `SessionTrace`, `WorkspaceNode`, `ControlMessage` (matches `routes/shells.rs`), `FileToken`. Generated by `scripts/openapi-to-types.sh` from the live OpenAPI spec |
| `@conusai/sdk` | `createConusSdk({ fetch, baseUrl, tokenProvider }) -> ConusSdk` with sub-clients `auth`, `capabilities`, `chat`, `chatApi`, `files`, `realtime`, `shells`, `threads`, `ui`, `workspaces`. Also `streamChat()`, `glyphFor()`, `EP` endpoint constants |
| `@conusai/ui` | Design system + Svelte 5 components. `tokens.css`, `foundry.css`, assets, theme stores (`themeStore`, `modeStore`, `featureFlags`, `toast`), utilities (`LiveAnnouncer`, markdown), components (`AppShell`, `TabStrip`, `WorkspaceTree`, `CommandPalette`, `RecorderControls`, `ToastHost`, `ThemeProvider`, `ThemeSwitcher`, `ArtifactPreview`, `CapabilityCard`), features (`AgentChatStream`, `AgentChatComposer`, `WorkspaceExplorer`, `ToolCallCard`, `createChatStream.svelte.ts` — Svelte 5 runes SSE consumer, `auth/LoginPanel`, `workspace/{Confirm,Move,NewNode,Share}Dialog`), and a **Capability Renderer Registry** (`capabilities/CapabilityRendererRegistry.svelte.ts`) so each app can register Svelte renderers per capability name |

---

## 7. Configuration & Environment

| Var | Default | Purpose |
|---|---|---|
| `CONUSAI_SERVER__HOST` | `0.0.0.0` | Bind address |
| `CONUSAI_SERVER__PORT` | `8080` | Listen port |
| `CONUSAI_CAPABILITIES_DIR` | `./capabilities` | Capability discovery root |
| `WORKSPACES_ROOT` | (unset → indexer disabled) | Tenant workspace mount; triggers `WorkspaceIndexer` + `RealFsWatcher` |
| `CONUSAI_UI_ASSETS` | (auto-detected) | Override UI assets directory |
| `CONUSAI_UI_TENANT_ID` | `dev` | Tenant ID used by the UI session |
| `CONUSAI_TEST_MODE` | — | `1` → all stores in-memory; no Qdrant/RustFS/redb |
| `CONUSAI_MAX_CAPABILITIES` | `64` | Admin limit: max registered capabilities |
| `CONUSAI_MAX_MANIFEST_BYTES` | `65536` | Admin limit: max manifest size |
| `CONUSAI_MAX_WASM_BYTES` | `8388608` | Admin limit: max WASM binary size (8 MiB) |
| `CONUSAI_MAX_TOOLS_PER_TURN` | `25` | Per-turn tool budget (RouterQuotaLayer) |
| `CONUSAI_MAX_INVOKES_PER_TURN` | `10` | Per-turn invoke budget |
| `CONUSAI_MAX_REPLAYS_PER_TURN` | `3` | Browser-shell replay quota per heartbeat |
| `CONUSAI_FEATURE_BROWSER_SHELL` | `0` | `1` enables `/v1/shells/{device_id}/control` + device endpoints |
| **`REDB_PATH`** | `/data/conusai.redb` | Path to the redb database file |
| **`QDRANT_URL`** | `http://qdrant:6334` | Qdrant gRPC endpoint |
| **`S3_ENDPOINT`** / `MINIO_ENDPOINT` | `http://rustfs:9000` | RustFS / MinIO endpoint |
| **`S3_BUCKET`** / `MINIO_BUCKET` | `workspace` | Storage bucket |
| `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` | `minioadmin` / `minioadmin` | Dev credentials |
| **`MARKER_URL`** | `http://marker-api:8080` | PDF → markdown service |
| `EMBEDDING_BACKEND` | `openai` | `openai` (default) ∣ `local` (fastembed, requires feature) ∣ unknown → error |
| `SEMANTIC_ROUTER_TOP_K` | `20` | Top-K capabilities per turn |
| `ANTHROPIC_API_KEY` | — | Required for all LLM calls |
| `OPENAI_API_KEY` | — | Used by `OpenAiEmbeddingService` + `VideoTranscriptionJob` |
| `JWT_SECRET` | — | HS256 key; unset → dev mode |
| `DEV_PASSWORD` | — | Login password in production mode |
| `SUPER_ADMIN_EMAILS` | — | CSV; logins with these emails get `UserRole::SuperAdmin` |
| `PLATFORM_ADMIN_TOKEN` | — | Bearer token for `/admin/capabilities/register` + device pairing |
| `API_KEYS` | — | `<blake3_hex>:<tenant_id>:<plan>` CSV |
| `UI_SESSION_KEY` | (dev secret) | HMAC key for session cookies (32 B in prod) |
| `WEB_ORIGIN` | `http://localhost:3000,http://localhost:5173,https://tauri.localhost,tauri://localhost` | Allowed CORS origins (CSV) |
| `OTLP_ENDPOINT` | — | OTel collector gRPC endpoint |
| `RUST_LOG` | — | `tracing` filter string |
| `CONUSAI_LLM__DEFAULT` / `…__ALIASES__*__MODEL` | — | LLM defaults via figment (TOML or env) |
| `CONUSAI_MCP_ALLOWED_HOSTS` | — | Host allowlist for MCP validation helpers |
| `CONUSAI_DEVICE_TOKEN` | — | Bootstrap token for the browser-shell in dev/E2E |

---

## 8. Startup & Request Lifecycle

### Gateway startup

1. `tokio::main` → telemetry init (JSON + OTLP + Prometheus single meter provider).
2. `AppState::from_env()` (§ 4.4).
3. Spawn capability-spec hot-reload listener.
4. Register `TranscribeVideoCapability`.
5. `verify_llm_providers` (warn-only).
6. `JobSchedulerService::start` (cron loop).
7. Optional `WorkspaceIndexer::index_once` + `RealFsWatcher::spawn` when `WORKSPACES_ROOT` is set.
8. Build router; apply middleware stack.
9. `axum::serve` on `${CONUSAI_SERVER__HOST}:${CONUSAI_SERVER__PORT}`.

### Request lifecycle

```
HTTP / WS request
  └─► axum router
        ├─ public_router      ──► /health, /v1/files/{token}, /v1/auth/login, /docs, /admin/capabilities/register
        ├─ /metrics           ──► Prometheus text (no auth)
        └─ protected_router (request_id → trace → api_key → tenant → plan)
              ├─ /v1/chat/completions
              ├─ /v1/agent/completions  (wrapped by RouterQuotaLayer)
              │     ├─ ConversationService::resolve_for_node  (lazy bind_thread)
              │     ├─ ContextBuilder::build_for_node(6000)
              │     ├─ ThreadStore::messages
              │     ├─ SemanticCapabilityRouter::select  ──► Qdrant ANN
              │     └─ Anthropic tool_use rounds (≤ plan.max_turns)
              │           ├─ ToolExecutor::invoke(registry, cap, tool, input, tenant)
              │           │     ├─ chain  → InvoiceProvider / ContractProvider / OcrProvider / PromptChainCapability
              │           │     ├─ dynamic → DynamicPromptCapability
              │           │     ├─ remote  → RemoteMcpCapability
              │           │     ├─ wasm    → WasmProvider
              │           │     ├─ mcp     → McpProvider
              │           │     └─ native  → BuiltinProvider / WorkspaceProvider / TraceReplayCapability
              │           └─ on end_turn:
              │                 ├─ ArtifactBridge::materialise(ToolOutput.artifacts)
              │                 ├─ ConversationService::append_message
              │                 └─ WorkspaceStore::index_content (last 30 msgs)
              ├─ /v1/capabilities, /v1/capabilities/search   (Qdrant + fallback)
              ├─ /mcp                                        (JSON-RPC 2.0)
              ├─ /v1/files                                   (RustFS multipart)
              ├─ /v1/audit
              ├─ /v1/workspaces                              (RedbMetadataStore + RustFsContentStore)
              ├─ /v1/tasks, /v1/tasks/{id}, /v1/tasks/{id}/sse
              ├─ /v1/threads/{id}/messages
              ├─ /api/realtime/workspace                     (WebSocket)
              └─ /v1/shells/{device_id}/control              (WebSocket, feature-gated)
        ├─ admin_router (require_super_admin_jwt)
        │     ├─ /admin/capabilities/*  CRUD + prompt versioning + namespace browsing
        │     ├─ /admin/devices/*       browser-shell pairing tokens
        │     └─ /admin/jobs/*, /admin/tasks
        └─ ui_router
              ├─ /, /login, /logout
              ├─ /ui/stream  /ui/upload  /ui/extract-invoice
              └─ /super-admin/*  (require_super_admin_session)
```

---

## 9. HTTP API Surface

### Public

| Method | Path | Purpose |
|---|---|---|
| GET | `/health` | Status / version / capability count |
| POST | `/v1/auth/login` | Exchange credentials for HS256 JWT |
| GET | `/v1/files/{token}` | Token-gated streaming download (1 h TTL) |
| POST | `/admin/capabilities/register` | Self-register remote MCP capability (Bearer `PLATFORM_ADMIN_TOKEN`) |
| GET | `/docs` | Swagger UI |
| GET | `/openapi.json` | OpenAPI 3.1 spec |
| GET | `/metrics` | Prometheus text format |

### Protected (Bearer JWT, `X-API-Key`, `conusai_session` cookie, or `X-Session-Token`)

`/v1/chat/completions`, `/v1/agent/completions`, `/v1/capabilities`, `/v1/capabilities/search`, `/mcp`, `/v1/files` (upload), `/v1/audit`, `/v1/workspaces[...]`, `/v1/tasks[...]`, `/v1/threads/{id}/messages`, `/api/realtime/workspace` (WS), `/v1/shells/{device_id}/control` (WS, device-token + feature-flag gated).

### Super-admin (JWT `role = super_admin`)

`/admin/capabilities[...]`, `/admin/capabilities/{name}/prompt[...]`, `/admin/capabilities/namespaces`, `/admin/jobs[...]`, `/admin/tasks`, `/admin/devices[...]`.

---

## 10. Security

- **Auth vectors (priority order in `auth::extractor`):** `conusai_session` cookie → `X-Session-Token` header (Tauri WKWebView) → HS256 Bearer JWT (`mw/tenant.rs`) → `X-API-Key` (BLAKE3-hashed against `API_KEYS`).
- **RBAC:** `UserRole { User, Admin, SuperAdmin }` in JWT/session; super-admin middleware enforces role on `/admin/*` and `/super-admin/*`.
- **Path safety:** `safe_join` rejects `..` in all tenant FS access (workspace bodies, builtin fs tools).
- **Storage isolation:** Qdrant payload filters on `tenant_id`; RustFS keys under `tenants/{tenant_id}/`; redb composite keys all start with `tenant_id`.
- **Workspace ACL:** private-by-default; per-node `shared_with`; non-owners receive `NotFound` (no existence leakage).
- **API keys / device tokens:** only blake3 hash stored; plaintext returned exactly once at issue time.
- **WASM sandboxing:** wasmtime 44 with Component Model; `MAX_WASM_SIZE_BYTES = 10 MB`; only allowlisted exports invoked.
- **CORS:** explicit `WEB_ORIGIN` allowlist (web 3000/5173, Tauri WKWebView origins); `allow_credentials: true`; exposes `X-Request-ID`.
- **Recorder PII redaction:** `RECORDER_BRIDGE_JS` strips values for fields matching `/password|ssn|cc-|card|cvv/i`; screenshots can be omitted for sensitive regions.
- **Tauri Stronghold** encrypts the device pairing token at rest on disk.
- **Request correlation:** `X-Request-ID` echoed in response and injected into JSON error bodies.

---

## 11. Observability

- **Structured logs:** JSON via `tracing-subscriber` (env-filter from `RUST_LOG`).
- **Distributed tracing:** W3C `traceparent`/`tracestate` propagation; OTLP export to otel-collector → Jaeger.
- **Metrics — OTel (OTLP + Prometheus):** single `SdkMeterProvider` with both readers.
  - `conusai.agent` meter: `agent.tool.invocations`, `agent.tool.errors`, `agent.tool.duration_ms`, `agent.llm.requests`, `agent.llm.input_tokens`, `agent.llm.output_tokens`, `gen_ai.semantic_router.cache_hit`, `gen_ai.semantic_router.top_k`, `gen_ai.semantic_router.distance`, `gen_ai.tool.calls`, `capability_router_select_seconds`, `capability_invoke_seconds`.
  - `conusai.storage` meter: `storage.request.duration_ms`, `storage.request.errors`.
- **Span attributes:** `tenant_id`, `plan`, `tool.cap`, `tool.name`, `error.type`, `gen_ai.system`, `gen_ai.request.model`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `thread_id`.
- **Prometheus endpoint:** `GET /metrics` (text/plain 0.0.4).
- **Healthcheck:** `GET /health` → `{status, version, capabilities}`.

---

## 12. Build, Test & Deploy

### Local development

```bash
# Infrastructure only (Qdrant + RustFS + redb volume)
./start.sh infra        # docker compose up qdrant rustfs rustfs-init

# Backend dev
just backend-dev        # cargo run -p agent-gateway

# Web dev
just web-dev            # pnpm --filter web dev   (Vite at :3000)

# Shell dev (desktop)
just shell-dev          # pnpm --filter browser-shell tauri dev

# Shell dev (iOS / Android)
just shell-ios-dev
just shell-android-dev
```

### Cargo builds

```bash
cargo build --release --bin agent-gateway
cargo build --release --bin evals
cargo build --release --target wasm32-wasip1 -p capability-example
```

### Docker

```bash
docker build -t conusai-gateway:0.4.0 -f apps/backend/Dockerfile .
docker compose --profile full up -d
# Optional profiles: observability, marker, web
```

### Build profiles

| Profile | `opt-level` | `lto` | `codegen-units` | `strip` |
|---|---|---|---|---|
| `release` | 3 | `thin` | 1 | `symbols` |
| `dev` | 0 | off | default | off |

### Quality gates (`just verify`)

```bash
cargo clippy --workspace -- -D warnings
pnpm -w lint           # biome + svelte-check
pnpm -w test           # vitest
cargo test --workspace
just types             # regenerate packages/types from OpenAPI
git diff --exit-code packages/types/src   # codegen drift gate
```

### E2E

```bash
pnpm e2e          # all Playwright projects
pnpm e2e:web      # web + Node SSR
pnpm e2e:ios      # iOS Safari (Playwright)
pnpm e2e:shell    # macOS Tauri shell (Playwright tauri-driver)
pnpm wdio:macos   # native macOS WebdriverIO + Appium
pnpm wdio:ios     # iOS WebView via WebdriverIO
pnpm wdio:ios-native   # iOS native via Appium XCUITest
```

---

## 13. Design Patterns

- **Single-process embedded storage:** one `RedbMetadataStore` instance fulfils `ThreadStore` + `WorkspaceStore` + `AuditStore` with atomic write transactions and snapshot reads — no Postgres, no schema migrations.
- **Vector workload externalised to Qdrant:** capability + content embeddings share a single Qdrant deployment; payload filters provide tenant/namespace/tag scoping.
- **Content vs metadata split:** raw markdown/blobs live in RustFS under `tenants/{id}/workspaces/{path}`; structured metadata in redb. `ArtifactBridge` keeps them in lockstep.
- **LLM abstraction layer:** `CompletionProvider` trait + `LlmRegistry` with 4-step alias resolution; adding a provider is a single file in `llm/providers/`.
- **Data-driven chain capabilities:** `LlmChainConfig` + `PromptTemplate` + `PromptChainCapability` — new LLM tools require only TOML.
- **Capability factories:** `McpFactory`, `WasmFactory`, `ChainFactory(llm)`, `BuiltinFactory`, plus `DynamicPromptFactory`, `RemoteMcpFactory`, `TraceReplayFactory`. Bulk path via `CapabilitySpecFactory` (DB-backed, batch-embedded, hot-reloaded).
- **Semantic routing with quota:** `SemanticCapabilityRouter` pre-selects top-K capabilities per turn; `RouterQuotaLayer` enforces the hard cap; `include_always` guarantees built-ins (e.g. workspace) survive pruning.
- **Artifact bridge:** every tool output may emit `ToolOutput.artifacts`; they materialise into workspace nodes + indexed content without per-tool wiring.
- **Cross-platform shell:** Tauri 2 wraps the same SvelteKit codebase for desktop and mobile; PII-safe DOM recorder injected into every tab webview; device tokens stored in Stronghold; WS control channel allows the server to drive replays.
- **Typed ID newtypes:** `ThreadId`, `NodeId`, `TenantId`, `UserId`, `ToolId` — compile-time safety; `serde(transparent)` wire format.
- **Multitenancy:** JWT/API-key/session auth → `TenantContext`; tenant-prefixed paths/keys/collections; plan-based token/turn/RPM caps; `UserRole` RBAC; `safe_join` path safety.
- **Observability by default:** JSON logs, OTel spans with W3C propagation, `#[instrument]` on every significant async method.
- **Scheduled + background jobs:** `ScheduledJob` + `tokio-cron-scheduler`; `BackgroundJob` + in-memory `JobExecutor` with SSE polling. Apalis/persistent migration-ready (trait unchanged).

---

## 14. Status

- **Version:** 0.4.0 (May 2026)
- **State:** operational; end-to-end verified per [verify/verify.md](verify/verify.md).

**Implemented** — multitenancy (JWT + API key + session cookie + Tauri `X-Session-Token`), `UserRole` (User/Admin/SuperAdmin), `CompletionProvider` + `LlmRegistry`, `AnthropicProvider` via `rig-core` 0.36, data-driven `PromptChainCapability`, `ConversationService`, super-admin capability CRUD API + Foundry UI, remote MCP self-registration (`services/current-time`), invoice + contract + OCR pipelines, TOML/JSON capability discovery, `CapabilityRegistry` + seven factories + bulk `CapabilitySpecFactory`, OpenAI-compatible chat, SSE streaming, tool-calling agent loop (blocking + streaming) with `RouterQuotaLayer`, MCP JSON-RPC 2.0, **Qdrant semantic capability search + content embeddings**, **RustFS S3-compatible content + file uploads**, **redb embedded metadata store** (threads/workspaces/audit), WASM execution (wasmtime 44 Component Model), Google Workspace manifest, evals framework (invoice + OCR), Jaeger/OTLP tracing, per-tenant rate limiting, `gen_ai.*` OTel span attributes, W3C traceparent propagation, native filesystem + cargo tools, workspace built-in capability (always-on), cargo-chef Docker image, hierarchical workspace, append-only audit log, Prometheus metrics, OpenAPI + Swagger UI, request-ID correlation, typed ID newtypes, CORS, scheduled jobs, background tasks + SSE, `TranscribeVideoCapability` (Whisper), `/v1/tasks`, `/v1/threads/{id}/messages`, `/api/realtime/workspace`, **cross-platform browser-shell (macOS, Windows, Linux, iOS, Android) with Tauri 2 + Stronghold device tokens + session recorder + tab strip + WS control + trace replay**, `/admin/jobs/*` REST API, `WorkspaceIndexer` + `RealFsWatcher`, `RealtimeService`, `ArtifactBridge` + canonical `ToolOutput`/`Artifact`, `runtime-echo` capability, `TraceReplayCapability` (+ Svelte renderer in the shell), database-backed capability specs with realtime hot-reload, **shared `@conusai/{types,sdk,ui}` packages**, **Marker PDF→markdown integration**, **fastembed local embeddings** (feature-gated).

**Reserved / future:** `Docker` capability kind, external MCP server federation, multi-instance deployment with shared redb-on-network or migration to embedded Sled cluster, audit retention/compaction, billing/quota enforcement, OIDC integration, multi-layer context budgeting, live document mode, additional LLM providers (OpenAI, Ollama, Bedrock), Apalis/Postgres-style job persistence, whisper-rs local transcription, Tauri Android signing automation (see `docs/ops/signing.md`).
