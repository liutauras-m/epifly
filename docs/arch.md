# ConusAI Platform — Granular Architecture Reference

> Companion to [arch.md](arch.md). This document is a **deep, code-level reference**: full file trees, every library + version, design principles, and exact integration patterns for `apps/web`, `apps/browser-shell`, `packages/ui`, and the Rust agent runtime (with explicit Rig 0.36 usage).
>
> Audit date: 2026-05-11 · Workspace versions: backend 0.3.1 (Rust) · UI 0.6.0 · browser-shell 0.4.0 · web 0.1.0

---

## 1. Repository Topology

### 1.1 Workspaces

The repo is a **dual workspace**:

- `pnpm-workspace.yaml` — TypeScript / Svelte packages.
- root `Cargo.toml` (resolver = "3", edition 2024, rust-version 1.95) — Rust crates.

Top-level layout:

```
conusai-platform/
├── Cargo.toml                  # Rust workspace root
├── package.json                # pnpm root
├── pnpm-workspace.yaml
├── turbo.json                  # Turborepo task graph
├── biome.json                  # Lint + format (Biome)
├── playwright.config.ts        # Cross-app Playwright config
├── docker-compose.yml          # Local Qdrant / RustFS / Postgres stack
├── justfile / Makefile         # Convenience tasks
├── apps/
│   ├── backend/                # Rust agent runtime + HTTP gateway
│   ├── web/                    # SvelteKit (Node adapter) — workshop UI
│   └── browser-shell/          # SvelteKit + Tauri 2 — desktop / iOS / Android
├── packages/
│   ├── ui/                     # @conusai/ui — shared design system + features
│   ├── sdk/                    # @conusai/sdk — typed API client
│   └── types/                  # @conusai/types — shared domain types
├── services/current-time/      # Sample MCP capability
├── workspaces/                 # Per-tenant data root (dev)
├── e2e/                        # Cross-app suites: ios, shell-macos, web, wdio
└── docs/
```

### 1.2 Cargo workspace members

```
apps/backend/crates/common         # error envelopes, config, SessionTrace, metrics
apps/backend/crates/agent-core     # Rig pipeline, capabilities, stores, embeddings
apps/backend/crates/jobs           # job queue + cron scheduler
apps/backend/crates/agent-gateway  # Axum HTTP gateway (binary)
apps/backend/evals                 # eval harness (separate workspace member)
apps/browser-shell/src-tauri       # Tauri 2 desktop/mobile shell (binary + lib)
```

### 1.3 pnpm workspace members

```
apps/web · apps/browser-shell · packages/ui · packages/sdk · packages/types
```

---

## 2. Core Libraries (Versions and Purpose)

### 2.1 Rust workspace dependencies

From root `Cargo.toml [workspace.dependencies]`:

| Crate | Version | Purpose |
| --- | --- | --- |
| `tokio` | 1 (`full`) | Async runtime |
| `tokio-stream` | 0.1 (`sync`) | SSE / channel streams |
| `rig-core` | **0.36** | LLM agent framework (providers, hooks, tools, streaming) |
| `redb` | 2 | Embedded KV metadata store |
| `postcard` | 1 (`alloc`) | Compact serialization for redb values |
| `qdrant-client` | 1 (`serde`) | Vector DB client (768-dim cosine) |
| `axum` | 0.8 (`ws`, `multipart`) | HTTP framework |
| `reqwest` | 0.13 (`json`, `stream`, `multipart`) | Outbound HTTP |
| `tower` / `tower-http` | 0.5 / 0.6 (`cors`, `trace`, `compression-br`, `fs`) | Middleware |
| `serde` / `serde_json` / `toml` | 1 / 1 / 0.8 | Serialization |
| `figment` | 0.10 (`env`, `toml`) | Config loader |
| `thiserror` / `anyhow` | 2 / 1 | Error handling |
| `tracing` + `tracing-subscriber` | 0.1 / 0.3 (`env-filter`, `json`) | Structured logs |
| `opentelemetry*` | 0.27 | OTLP traces + metrics |
| `tracing-opentelemetry` | 0.28 | Tracing → OTel bridge |
| `prometheus` / `opentelemetry-prometheus` | 0.13 / 0.27 | Metrics scrape endpoint |
| `wasmtime` + `wasmtime-wasi` | **44** (`component-model`) | WASI p2 capability execution |
| `jsonwebtoken` | 9 | JWT auth |
| `sha2` / `blake3` / `hmac` | 0.10 / 1 / 0.12 | Crypto + cache keys |
| `schemars` | 0.8 (`derive`, `chrono`) | JSON Schema for tool I/O |
| `base64` | 0.22 | Image / token encoding |
| `object_store` | 0.11 (`aws`) | RustFS / S3 / MinIO content store |
| `fastembed` | 5 (optional) | Local embeddings (`local-embeddings` feature) |
| `uuid` / `ulid` / `chrono` | 1 / 1.1 / 0.4 | IDs + time |
| `bytes` / `futures` / `async-trait` / `bon` | 1 / 0.3 / 0.1 / 3 | Async + builder ergonomics |
| `moka` | 0.12 (`future`) | In-process cache (semantic router) |
| `clap` | 4 (`derive`) | CLI |
| `tokio-cron-scheduler` | 0.13 | Job scheduler |
| `utoipa` + `utoipa-swagger-ui` | 5 / 9 (`axum`) | OpenAPI spec + Swagger UI |
| `proptest` / `wiremock` | 1 / 0.6 | Property + HTTP mock testing |

Release profile: `opt-level=3`, `lto="thin"`, `codegen-units=1`, `strip="symbols"`.

### 2.2 `apps/browser-shell/src-tauri` Rust deps

| Crate | Version | Purpose |
| --- | --- | --- |
| `tauri` | 2 (`unstable`) | Cross-platform shell runtime |
| `tauri-plugin-dialog` | 2 | Native open/save dialogs |
| `tauri-plugin-stronghold` | 2 | Encrypted secret vault (key = blake3 of password) |
| `tauri-plugin-http` | 2 | Native HTTP client bypassing CORS |
| `tauri-plugin-updater` | 2 (desktop only, optional `updater` feature) | Auto-update |
| `tauri-plugin-webdriver-automation` | 0.1.3 (debug + macOS, optional `e2e` feature) | W3C WebDriver server for WKWebView |
| `futures-util` | 0.3 | SSE byte-stream consumption |
| `reqwest`, `tokio`, `serde`, `serde_json`, `tracing`, `ulid`, `chrono`, `blake3`, `anyhow`, `base64` | workspace | Shared |

Crate types: `["staticlib", "cdylib", "rlib"]` — supports iOS / Android linking and a desktop `[[bin]]`.

### 2.3 `packages/ui` deps (`@conusai/ui` v0.6.0)

| Package | Version | Role |
| --- | --- | --- |
| `svelte` | ^5 (peer + dev) | Component framework (runes) |
| `@sveltejs/vite-plugin-svelte` | ^4 | Build |
| `vite` | ^6 | Build / HMR |
| `vitest` + `@testing-library/svelte` | ^2 / ^5 | Unit tests |
| `svelte-check` | ^4 | Type check |
| `typescript` | ^5.7 | TS |
| `@conusai/types` | workspace:* | Shared types |

Exports map: `.`, `./tokens.css`, `./foundry.css`, `./assets/*`, `./capabilities`, `./stores`, `./utils`, `./features`, `./motion`. `sideEffects: ["**/*.css"]` so CSS is preserved by tree-shakers.

### 2.4 `apps/web` deps (v0.1.0)

| Package | Version | Role |
| --- | --- | --- |
| `@sveltejs/kit` | ^2.21 | Framework |
| `@sveltejs/adapter-node` | ^5.2.12 | SSR Node server |
| `svelte` | ^5.33 | Runes |
| `@conusai/{ui,sdk,types}` | workspace | Shared |
| `@playwright/test` | ^1.49 | E2E |
| `vitest` (+ `@vitest/ui`) | ^2.1.9 | Unit |
| `vite` | ^6.3.5 | Build |

### 2.5 `apps/browser-shell` deps (v0.4.0)

| Package | Version | Role |
| --- | --- | --- |
| `@sveltejs/kit` + `@sveltejs/adapter-static` | ^2.21 / ^3 | Pre-rendered static for Tauri load |
| `svelte` | ^5.33 | Runes |
| `@tauri-apps/api` | ^2 | JS↔Rust bridge |
| `@tauri-apps/plugin-dialog` | ^2 | Dialogs |
| `@tauri-apps/plugin-stronghold` | ^2 | Token storage |
| `@tauri-apps/cli` | ^2 (dev) | CLI |
| `vite-plugin-static-copy` | ^1 | Static asset copy |
| `@conusai/{ui,sdk,types}` | workspace | Shared |

---

## 3. Architectural Principles

These are the **hard rules** the codebase encodes:

1. **Single source of LLM access.** All model calls go through `agent-core::llm::LlmRegistry`. No route, chain, or memory module constructs a Rig provider client directly. Adding a provider = one file in `llm/providers/`.
2. **Semantic prefilter, never the full tool catalog.** `SemanticCapabilityRouter` enforces a hard cap (≤ 50, default top-K = 20) of tool definitions per agent turn. Tool selection is ANN over capability embeddings stored in Qdrant.
3. **Centralised UI in `packages/ui`.** Web and shell consume the same components, stores, motion primitives, and `createChatStream` — diverging only in transport (`tauriStreamFn` vs `sdk.chat.stream`).
4. **Svelte 5 runes-only.** Stores are `*.svelte.ts` files exporting `$state` / `$derived` factories — no Svelte 4 writable stores, no Pinia-style singletons.
5. **Native bridge over WebKit limitations.** SSE is proxied through Tauri events (`chat:chunk:<id>`) because WKWebView buffers `text/event-stream`. Recorder DOM events are forwarded through a single injected JS bridge.
6. **Dual-token shell auth.** Device identity (Stronghold-stored `X-Device-Token`) is independent from UI session (`X-Session-Token` HMAC cookie). Either can be rotated without invalidating the other.
7. **Reduced motion is a first-class token.** `@media (prefers-reduced-motion: reduce)` clamps every animation to 0.01ms in `tokens.css`; motion primitives also short-circuit programmatically.
8. **Tenant boundaries are typed.** `TenantContext` carries `tenant_id`, `plan`, `preferred_model`, and `safe_path()`. Plan limits (`max_tokens`, `max_turns`, `default_alias`) clamp every Rig agent build.
9. **OpenAPI is generated from code.** `utoipa` derive on every route + `SecurityAddon` modifier emit a single Swagger document; UI can regenerate types via `scripts/openapi-to-types.sh`.
10. **No hidden network in startup.** `verify_llm_providers` validates aliases at boot but performs no outbound call; capability registration in shell uses the `shell-ready` event after Stronghold loads.

---

## 4. Backend Runtime — `apps/backend`

### 4.1 Crate map

```
apps/backend/
├── Cargo.toml
├── Dockerfile
├── start.sh / start-verify.sh / stop.sh
├── crates/
│   ├── common/                    # error envelopes, config, SessionTrace, metrics
│   ├── agent-core/                # Rig pipeline + capabilities + stores
│   ├── agent-gateway/             # Axum binary
│   └── jobs/                      # job queue + cron scheduler
├── evals/                         # eval harness
├── capabilities/                  # TOML manifests for chain/MCP/builtin capabilities
└── scripts/
```

### 4.2 `agent-core` — full file structure

```
agent-core/src/
├── lib.rs
├── agent/
│   ├── mod.rs
│   ├── builder.rs          # AgentBuilder + Agent (Rig anthropic agent wrapper)
│   ├── runtime.rs          # AgentRuntime + map_rig_error
│   └── hooks.rs            # TracingHook, PermissionHook (Rig PromptHook impls)
├── llm/
│   ├── mod.rs              # re-exports
│   ├── types.rs            # LlmRequest/Response/Stream/Chunk/Binding
│   ├── error.rs            # LlmError
│   ├── provider.rs         # CompletionProvider trait
│   ├── registry.rs         # LlmRegistry, verify_llm_providers
│   ├── streaming.rs        # openai_sse_to_stream helper
│   └── providers/
│       ├── mod.rs
│       └── anthropic.rs    # AnthropicProvider (wraps rig::providers::anthropic)
├── chains/
│   ├── mod.rs
│   ├── contract.rs         # ContractPipeline (Claude vision extractor)
│   ├── extraction.rs
│   ├── invoice.rs
│   ├── llm_chain.rs        # PromptChainCapability — TOML-driven LLM chain
│   ├── dynamic_prompt.rs   # DynamicPromptCapability (manifest-only prompt)
│   └── executor.rs         # run_chain shared core
├── capabilities/
│   ├── mod.rs
│   ├── manifest.rs         # ToolManifest + LlmChainConfig
│   ├── card.rs             # CapabilityCard (manifest + provider + path)
│   ├── provider.rs         # CapabilityProvider, CapabilityFactory, BulkCapabilityFactory
│   ├── registry.rs         # CapabilityRegistry (with_default_factories / with_all_factories)
│   ├── namespace.rs        # NamespaceFilter
│   ├── discovery.rs        # filesystem discovery of TOML manifests
│   ├── embedding.rs        # capability text → vector for semantic router
│   ├── executor.rs
│   ├── semantic_router.rs  # SemanticCapabilityRouter (top-K, moka cache, blake3 keys)
│   ├── trace_replay.rs     # TraceReplayFactory
│   ├── store.rs
│   ├── validator.rs
│   ├── wasm_loader.rs      # wasmtime 44 component-model loader
│   ├── mcp_adapter.rs      # MCP protocol adapter
│   ├── admin.rs
│   ├── providers/
│   │   ├── builtin.rs / chain.rs / mcp.rs / wasm.rs / dynamic_prompt.rs
│   └── builtin/
│       ├── card.rs / fs.rs / cargo.rs
├── context/
│   ├── tenant.rs           # TenantContext + plan limits + safe_path
│   └── conversation.rs
├── memory/
│   ├── context_builder.rs
│   └── truncator.rs
├── store/
│   ├── redb_metadata.rs    # KV (postcard-encoded)
│   ├── qdrant_vector.rs    # 768-dim cosine
│   ├── rustfs_content.rs   # object_store (S3/MinIO)
│   └── marker.rs
├── vector_store/mod.rs
├── indexing/
│   ├── coco_indexer.rs
│   ├── embedding_service.rs       # EmbeddingService trait
│   ├── local_embedding_service.rs # fastembed (feature-gated)
│   └── real_fs_watcher.rs
├── prompt/mod.rs            # PromptTemplate
├── realtime/mod.rs
└── bridge/artifact_bridge.rs
```

### 4.3 `agent-gateway` — full file structure

```
agent-gateway/src/
├── main.rs                # binary entry
├── state.rs               # AppState (registry, llm, jobs, stores)
├── auth/                  # JWT + cookie + device-token verification
├── capabilities/          # capability bootstrapping wiring
├── mw/                    # tower middleware (RouterQuotaLayer, etc.)
├── ui/                    # /ui/* HTML + handlers (askama templates)
│   ├── routes.rs / session.rs / handlers/
└── routes/
    ├── mod.rs             # OpenAPI assembly + SecurityAddon
    ├── auth.rs            # /v1/auth/login → JWT
    ├── chat.rs            # /v1/chat/completions (OpenAI-compatible, streaming)
    ├── agent.rs           # /v1/agent/completions (semantic-router agent)
    ├── threads.rs         # /v1/threads, /v1/threads/{id}/messages
    ├── workspaces.rs      # /v1/workspaces (multipart upload, tree, move)
    ├── files.rs           # /v1/files (multipart, FileToken)
    ├── capabilities.rs    # /v1/capabilities list/search
    ├── tasks.rs           # /v1/tasks
    ├── search.rs          # /v1/search (vector + lexical)
    ├── shells.rs          # /v1/shells (device session)
    ├── mcp.rs             # /mcp/*
    ├── realtime.rs        # WebSocket /v1/realtime
    ├── audit.rs / health.rs
    ├── admin_capabilities.rs   # /admin/capabilities/register
    ├── admin_devices.rs        # /admin/devices issue/list
    └── admin_jobs.rs
```

OpenAPI security schemes registered in `routes/mod.rs::SecurityAddon`:

- `bearer_auth` — HTTP Bearer JWT
- `api_key_auth` — `X-API-Key` header
- `cookie_auth` — `conusai_session` cookie

### 4.4 Rig 0.36 Usage — Exhaustive

ConusAI uses Rig as the **agent execution engine**. All Rig usage is concentrated in `agent-core`.

#### 4.4.1 Provider clients

`llm/providers/anthropic.rs::AnthropicProvider` constructs `rig::providers::anthropic::Client` (`from_env` → `ANTHROPIC_API_KEY`). Implements ConusAI's `CompletionProvider` trait by delegating to:

- `client.completion_model(model_id)` → `rig::providers::anthropic::completion::CompletionModel`
- `model.completion_request(rig::message::Message::User { … })` — Rig builder
- `.temperature(f64)`, `.max_tokens(u64)` — Rig setters
- `model.completion(rig_req).await` — non-stream
- For streaming: `rig::streaming::StreamedAssistantContent` consumed via `futures::StreamExt`, mapped to `LlmChunk` and re-emitted as `Pin<Box<dyn Stream>>` (`LlmStream`).

Rig trait imports needed in scope:

```rust
use rig::client::ProviderClient;
use rig::client::completion::CompletionClient;
use rig::completion::{CompletionModel, Prompt, ToolDefinition};
use rig::message::{Message, UserContent, ImageMediaType, AssistantContent};
use rig::OneOrMany;
use rig::streaming::StreamedAssistantContent;
use rig::tool::{ToolDyn, ToolError};
use rig::agent::{HookAction, PromptHook, ToolCallHookAction};
```

#### 4.4.2 Agent builder (`agent/builder.rs`)

```rust
let inner = client.agent(&self.model)
    .preamble(&self.preamble)
    .max_tokens(max_tokens)
    .build();
```

`Agent::prompt`:

- Builds `TracingHook` (Rig `PromptHook<M>` impl) with tenant + plan + thread.
- Resolves `max_turns` from `tenant.plan.max_turns()` (default 10).
- If a `SemanticCapabilityRouter` is wired:
  - Calls `router.rig_tools_for_prompt(text, tenant)` — returns `Vec<Box<dyn rig::tool::ToolDyn>>`.
  - Rebuilds a fresh agent with `.tools(tools)` so only top-K capabilities appear in the LLM tool catalog.
- Calls `agent.prompt(text).max_turns(n).with_hook(hook).await`.

Default model: `claude-sonnet-4-6` (`AgentBuilder::default`).

#### 4.4.3 Hooks (`agent/hooks.rs`)

`TracingHook` implements `rig::agent::PromptHook<M: CompletionModel>`:

- `on_completion_call` — `info!` with `tenant_id`, `plan`, `thread_id`; returns `HookAction::cont()`.
- `on_tool_call` — logs `tool_name`, `tool_call_id`, `internal_call_id`, `args`; returns `ToolCallHookAction::cont()`.
- `on_tool_result` — logs `tool_name`, `result_bytes`.

`PermissionHook` returns `ToolCallHookAction::Skip { reason }` when a tool is not in `allowed_tools` — the LLM receives a graceful denial rather than an error.

#### 4.4.4 Semantic router → Rig tools

`capabilities/semantic_router.rs::SemanticCapabilityRouter`:

- Owns `Arc<Mutex<CapabilityRegistry>>`, `Arc<QdrantVectorStore>`, `Arc<dyn EmbeddingService>`.
- Cache: `moka::future::Cache<[u8; 32], Arc<CachedResult>>` keyed by blake3 of the query (TTL 60s).
- Default config: `top_k=20`, `max_distance=0.38` (tuned for nomic-embed-text), `include_always` for hard-pinned tools.
- Per turn, returns `Vec<rig::completion::ToolDefinition>` plus `Vec<Box<dyn rig::tool::ToolDyn>>` adapters that route Rig invocations back into `CapabilityProvider::invoke`.

#### 4.4.5 Chains (`chains/executor.rs`)

`run_chain` builds Rig messages directly:

```rust
messages.push(Message::System { content });
messages.push(Message::User {
    content: OneOrMany::one(UserContent::text(rendered_prompt)),
});
```

— then resolves the `LlmRegistry` provider for `cfg.model` (alias or model id) and dispatches via `CompletionProvider::complete`. JSON output is auto-parsed; non-JSON is wrapped as `{ "result": text }`.

`ContractPipeline` (`chains/contract.rs`) bypasses the registry to use Claude vision directly:

```rust
UserContent::image_base64(b64, Some(ImageMediaType::PNG), None)
```

Defaults to model `claude-opus-4-7` and uses `client.completion_model(...)` from `rig::providers::anthropic`.

#### 4.4.6 Error mapping

`agent/runtime.rs::map_rig_error` converts the `Display` output of Rig errors (which are not `'static + Send` across releases) to `HttpError`:

| Substring | Mapped to |
| --- | --- |
| `max turns`, `maxturns`, `maximum number of turns` | `HttpError::agent("agent reached max-turns limit: …")` |
| `rate limit`, `429` | `HttpError::rate_limit(Some(60))` |
| `unauthorized`, `authentication`, `api key`, `401` | `HttpError::auth(...)` |
| `tool` + (`error`/`fail`/`not found`) | `HttpError::agent("tool execution failed: …")` |
| _otherwise_ | `HttpError::agent(...)` |

### 4.5 LLM Registry resolution order

`LlmRegistry::resolve_binding(alias_or_model, tenant)`:

1. `tenant.preferred_model` — if matches an alias, use it; if it looks like a concrete model id, bind to default provider.
2. Caller-supplied `alias_or_model` — if matches an alias, use it.
3. `tenant.plan.default_alias()` — plan-level fallback.
4. `self.default` — global registry default from `[llm].default`.

Built from `LlmConfig` aliases:

```toml
[llm]
default = "fast"
[llm.aliases.fast]
provider = "anthropic"
model = "claude-haiku-4-1"
[llm.aliases.smart]
provider = "anthropic"
model = "claude-sonnet-4-6"
```

`verify_llm_providers` checks every alias resolves to a registered provider at boot — no network.

### 4.6 Capability factory chain

`CapabilityRegistry::with_default_factories(llm)` registers (in order):

1. `McpFactory` — remote MCP servers.
2. `WasmFactory` — wasmtime 44 component model (`.wasm` modules in `capabilities/`).
3. `ChainFactory::new(llm)` — instantiates `PromptChainCapability` for any manifest with `kind = "chain"` and a `[chain]` block.
4. `BuiltinFactory` — `fs`, `cargo`, etc.

`with_all_factories` additionally registers `DynamicPromptFactory` and `TraceReplayFactory`.

Each factory implements `CapabilityFactory::supports(manifest) -> bool` and `build(manifest) -> Arc<dyn CapabilityProvider>`. A `BulkCapabilityFactory` may load many cards in one boot pass (`run_bulk_load`).

### 4.7 Storage backplane

| Concern | Backend | Module |
| --- | --- | --- |
| Metadata KV | redb 2 (postcard) | `store/redb_metadata.rs` |
| Vector ANN | Qdrant 1.x (768-dim cosine) | `store/qdrant_vector.rs` |
| Object content | RustFS / MinIO / S3 (`object_store` 0.11) | `store/rustfs_content.rs` |
| Local embeddings | fastembed 5 (feature `local-embeddings`) | `indexing/local_embedding_service.rs` |
| Cross-instance index marker | `store/marker.rs` | |

---

## 5. Shared UI Package — `packages/ui`

### 5.1 Full file tree

```
packages/ui/
├── package.json (v0.6.0)
├── scripts/assets-verify.js
├── tests/                        # Vitest specs
└── src/lib/
    ├── index.ts                  # public re-exports
    ├── tokens.css                # color/space/motion CSS variables
    ├── foundry.css               # full design system + reset + utility classes
    ├── components/
    │   ├── AppShell.svelte
    │   ├── ArtifactPreview.svelte
    │   ├── CapabilityCard.svelte
    │   ├── CommandPalette.svelte
    │   ├── RecorderControls.svelte
    │   ├── TabStrip.svelte               # exports type Tab
    │   ├── ThemeProvider.svelte
    │   ├── ThemeScript.ts                # THEME_SCRIPT (FOUC-free hydration)
    │   ├── ThemeSwitcher.svelte
    │   ├── ToastHost.svelte              # exports type Toast
    │   └── WorkspaceTree.svelte
    ├── features/
    │   ├── AgentChatComposer.svelte      # exports type Attachment
    │   ├── AgentChatStream.svelte        # exports types ChatMessage, ToolCardEntry
    │   ├── ToolCallCard.svelte
    │   ├── WorkspaceExplorer.svelte
    │   ├── createChatStream.svelte.ts    # streaming state machine
    │   ├── auth/LoginPanel.svelte
    │   ├── workspace/{Confirm,Move,NewNode,Share}Dialog.svelte
    │   └── index.ts
    ├── motion/
    │   ├── spring.ts            # springAnimate(opts: SpringOpts)
    │   ├── flip.ts              # recordRect, playFlip
    │   ├── stagger.ts           # stagger(entries, perItemMs)
    │   ├── tap.ts               # platform-aware tap (ripple on Android, scale otherwise)
    │   ├── viewTransition.ts    # startViewTransition with fallback
    │   └── index.ts
    ├── stores/
    │   ├── themeStore.svelte.ts        # createThemeStore + localStorageAdapter
    │   ├── featureFlags.svelte.ts      # createFeatureFlags ({recorder, tabs, traceReplay})
    │   ├── toast.svelte.ts             # toasts singleton, ToastKind
    │   ├── modeStore.svelte.ts         # AppMode (web | shell)
    │   ├── recents.svelte.ts           # recentsStore
    │   ├── breadcrumbs.svelte.ts       # breadcrumbsStore
    │   └── index.ts
    ├── capabilities/
    │   ├── CapabilityRendererRegistry.ts        # plain Map-based registry
    │   ├── CapabilityRendererRegistry.svelte.ts # provide/use via Svelte context
    │   └── index.ts
    ├── utils/
    │   ├── LiveAnnouncer.svelte        # aria-live region
    │   ├── actions.ts                  # autoGrow textarea action
    │   ├── motion-prefs.ts             # prefersReducedMotion()
    │   ├── md.ts                       # safe Markdown render
    │   └── index.ts
    └── assets/
        ├── fonts/Geist-Variable.woff2
        ├── fonts/GeistMono-Variable.woff2
        └── images/                     # logos, favicons, brand variants
```

### 5.2 Design system tokens (`tokens.css` + `foundry.css`)

Themes selected via `:root[data-theme="paper" | "forge"]` (set by `ThemeProvider` + `ThemeScript`).

| Token | Light (paper) | Dark (forge) | Notes |
| --- | --- | --- | --- |
| `--ink` / `--paper` | `#111` / `#F8F8F8` | `#F8F8F8` / `#111` | Inverted neutrals |
| `--ink-2/3`, `--paper-2/3` | grayscale ladder | grayscale ladder | Surface depth |
| `--rule`, `--seam` | `#E0E0E0` / `#C8C8C8` | `#2A2A2A` / `#3A3A3A` | Borders |
| `--ember` | `#FF6200` | same | Brand orange |
| `--ember-2` | `#E05500` | `#FF7A20` | Brighter on dark |
| `--ember-soft` / `--ember-glow` | rgba 0.10 / 0.22 | 0.12 / 0.28 | Halos |
| `--cyan` / `--cyan-soft` | `#00D4FF` / 0.10 | same / 0.12 | Secondary accent |
| `--success` / `--danger` | `#1a7f4b` / `#b32400` | `#22a060` / `#e03000` | Semantic |
| `--shadow-sm/md`, `--backdrop` | 0.08 / 0.12 / 0.40 | 0.30 / 0.50 / 0.60 | Overlays |
| `--poster-gradient` | `linear-gradient(135deg, #FF6200 0%, #E05500 60%, #111 100%)` | — | Login poster |

Type scale (`foundry.css`):

```
--t-display: clamp(40px, 5.4vw, 56px)
--t-h1: 28  --t-h2: 20  --t-body: 15  --t-meta: 13  --t-label: 11  --t-mono: 13
```

Spacing: `--s-1..s-8` = `4 8 12 16 24 32 48 64`. Layout: `--rail: 240px`, `--gutter: 64px`, `--composer-w: 720px`.

Motion:

```
--ease-out:    cubic-bezier(0.22, 1, 0.36, 1)   /* exits */
--ease-in:     cubic-bezier(0.6, 0, 0.7, 0.2)
--ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1)
--dur-1..4: 120 / 200 / 320 / 520 ms
```

Radii: `--r-xs..r-xl` = `6 / 10 / 14 / 20 / 28`, plus `--r-full: 9999px`.

Reduced motion guard:

```css
@media (prefers-reduced-motion: reduce) {
  * { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; }
}
```

Fonts: `Geist-Variable.woff2` + `GeistMono-Variable.woff2` self-hosted with `font-display: swap` — CSP-safe and works offline inside Tauri.

### 5.3 `createChatStream.svelte.ts` (state machine)

Public surface:

```ts
createChatStream(sdk: ConusSdk, options?: { streamFn?: CustomStreamFn }) → {
  messages: ChatMessage[]            // $state
  toolCards: Map<string, ToolCardEntry>
  inFlight: boolean
  activeThreadId: string | null
  send(prompt, { workspaceNodeId, attachmentIds, onThreadId })
  abort()
  newSession()
  loadThread(threadId)
}
```

Behaviors:

- `INACTIVITY_TIMEOUT_MS = 45_000`; a `setInterval(5000)` aborts on stalled streams.
- Word animation: deltas accumulate in `wordAccum`, flushed via `requestAnimationFrame`. Each word becomes `{ t, id, delay: i * 18 }` for staggered fade-in.
- Tool lifecycle: `tool_start` → `running` card; `tool_result` → parses JSON, marks `error` if `{ error }` / `{ status: "error" }` / starts with `Error:`, else `success`.
- Thread id capture via `delta.kind === "thread_id"` + `onThreadId(id)` callback.
- Optional `streamFn` lets the shell substitute `streamChatTauri` for `sdk.chat.stream`.

### 5.4 Capability renderer registry pattern

Two implementations:

- `CapabilityRendererRegistry.ts` — pure Map: `register / unregister / get(card)`.
- `CapabilityRendererRegistry.svelte.ts` — Svelte context wrappers: `provideCapabilityRendererRegistry()` / `useCapabilityRendererRegistry()`.

`apps/web/src/routes/+page.svelte` calls `provideCapabilityRendererRegistry()` at the route root. `apps/browser-shell/src/routes/+page.svelte` registers the `trace.replay` renderer at startup.

### 5.5 Motion primitives

| Function | File | Notes |
| --- | --- | --- |
| `springAnimate({ from, to, stiffness, damping, onUpdate })` | `motion/spring.ts` | RAF-driven; respects `prefers-reduced-motion` (instant snap). |
| `recordRect(el)` + `playFlip(el, prevRect)` | `motion/flip.ts` | FLIP layout transition. |
| `stagger(items, perItemMs)` | `motion/stagger.ts` | Returns delay per index. |
| `tap(node, opts)` | `motion/tap.ts` | Action: ripple on Android UA, scale-down on others. |
| `startViewTransition(fn)` | `motion/viewTransition.ts` | Falls back to direct `fn()` when API absent. |

### 5.6 Stores

All stores follow factory pattern returning a `$state`-backed object plus mutation methods. Notable:

- `createThemeStore({ adapter, defaultTheme })` + `localStorageAdapter` — theme persists across sessions.
- `createFeatureFlags({ recorder, tabs, traceReplay })` — central flag gate (defined but underused at runtime today).
- `recentsStore` — recent thread metadata; consumed by `DrawerRecentChats`.
- `breadcrumbsStore` — current node trail.
- `modeStore` — `'web' | 'shell'`; lets components branch on host without checking `window.__TAURI__`.
- `toasts` — singleton with `success/info/warn/error/dismiss`.

---

## 6. Web App — `apps/web`

### 6.1 File tree

```
apps/web/
├── package.json (v0.1.0)
├── svelte.config.js          # adapter-node, csrf.checkOrigin = false (manual in hooks)
├── vite.config.ts
├── playwright.config.ts
├── e2e/
│   ├── smoke.test.ts
│   └── …
├── static/
└── src/
    ├── app.html
    ├── app.d.ts              # locals.user typed as SessionUser | null
    ├── hooks.server.ts       # CSRF + session resolution
    ├── lib/
    │   ├── sdk.ts            # createConusSdk({ baseUrl, fetch, headers })
    │   └── server/
    │       ├── env.ts        # validated env (UI_SESSION_KEY, BACKEND_AUTH_LOGIN_URL, …)
    │       └── session.ts    # COOKIE_NAME, sign/verify, SessionAdapter, LocalHmacAdapter, BackendJwtAdapter
    ├── routes/
    │   ├── +layout.server.ts # redirect → /login if !locals.user
    │   ├── +layout.svelte    # imports @conusai/ui/foundry.css, mounts ThemeProvider + ToastHost
    │   ├── +page.server.ts   # parallel load of threads, capabilities, workspace tree
    │   ├── +page.svelte      # Workshop layout
    │   ├── +error.svelte
    │   ├── login/+page.svelte + +page.server.ts (form action calls sessionAdapter.issue)
    │   └── logout/+page.server.ts (cookie delete)
    └── tests/                # Vitest — currently targets ../lib/api/stream which is not present
```

### 6.2 Session machinery (`lib/server/session.ts`)

- Cookie: `conusai_session`, TTL `24 * 3600s`.
- `LocalHmacAdapter` (default): `sign(name, plan)` → `<base64url(payload)>.<base64url(hmacSHA256)>`.
- `BackendJwtAdapter`: activated when `BACKEND_AUTH_LOGIN_URL` is set; calls backend `/auth/login`, decodes JWT payload locally without re-verifying (gateway already verified on each call).
- `getKey()` throws in production if `UI_SESSION_KEY` is missing; dev fallback is hard-coded.

### 6.3 Request pipeline (`hooks.server.ts`)

1. **Manual CSRF**: SvelteKit's `csrf.checkOrigin` is disabled in `svelte.config.js`. Hook re-enforces origin = host on non-GET/HEAD requests **except** for paths starting with `/v1`, `/api`, `/ui`, `/mcp`, `/admin` (these are backend proxies).
2. **Session**: reads `conusai_session` cookie, calls `verify(raw)` (HMAC adapter), assigns `locals.user`.

`+layout.server.ts` redirects to `/login` when `locals.user` is null.

### 6.4 Main page composition (`routes/+page.svelte`)

Wired imports:

```ts
provideCapabilityRendererRegistry()                  // from @conusai/ui/capabilities
const chatStream = createChatStream(sdk)             // no custom streamFn — uses sdk.chat.stream
<WorkspaceExplorer {sdk} bind:nodes bind:selectedNodeId {onSelectNode} />
<AgentChatComposer bind:value onsubmit onUpload />
<AgentChatStream messages={chatStream.messages} toolCards={chatStream.toolCards} />
<ThemeSwitcher />
```

Behavior:

- Selecting a `kind === 'conversation'` node with `metadata.thread_id` calls `chatStream.loadThread(threadId)`.
- `Cmd/Ctrl+N` resets stream + collapses to greeting screen.
- File upload through `sdk.workspaces.upload(file)`; failures surface via `toasts.error`.
- Sidebar slides on mobile via CSS media queries.

### 6.5 Server load (`routes/+page.server.ts`)

Parallel SDK fan-out:

- `sdk.threads.list()` → recents
- `sdk.capabilities.list()` → glyph + count tiles
- `sdk.workspaces.tree()` → initial node forest

Returns safe defaults if any call fails so the page always renders.

---

## 7. Browser Shell — `apps/browser-shell`

### 7.1 File tree

```
apps/browser-shell/
├── package.json (v0.4.0)
├── svelte.config.js          # adapter-static (Tauri loads pre-rendered HTML)
├── vite.config.ts            # vite-plugin-static-copy for fonts/assets into Tauri bundle
├── static/                   # icons, splash
├── src/
│   ├── app.html
│   ├── routes/
│   │   ├── +layout.svelte    # foundry.css + ThemeProvider, mode = 'shell'
│   │   └── +page.svelte      # registers trace.replay renderer; mounts MobileShell
│   └── lib/
│       ├── sdk.ts            # createConusSdk with x-session-token + device-token headers
│       ├── tauri-stream.ts   # streamChatTauri SSE bridge
│       ├── TraceReplayCapability.svelte
│       └── mobile/
│           ├── MobileShell.svelte
│           ├── platform/detect.ts          # setPlatformTag (data-platform on <html>)
│           ├── stores/
│           │   ├── drawer.svelte.ts        # drawerStore.{open,close,toggle}
│           │   ├── screen.svelte.ts        # screenStore.active = chat|capabilities|artifacts
│           │   └── sheet.svelte.ts         # bottom-sheet visibility
│           ├── chrome/
│           │   ├── MobileTopBar.svelte
│           │   ├── MobileDrawer.svelte
│           │   └── MobileBottomSheet.svelte
│           ├── parts/
│           │   ├── DrawerProfileHeader.svelte
│           │   ├── DrawerWorkspaceTree.svelte
│           │   ├── DrawerRecentChats.svelte
│           │   ├── ProfileSheet.svelte
│           │   ├── AttachmentSheet.svelte
│           │   ├── CapabilityRow.svelte
│           │   ├── CapabilityDetailSheet.svelte
│           │   ├── ArtifactRow.svelte
│           │   ├── Breadcrumbs.svelte
│           │   ├── ContextChip.svelte
│           │   ├── SuggestionChips.svelte
│           │   ├── WorkspaceTreeRow.svelte
│           │   └── WorkspaceCreateMenu.svelte
│           └── screens/
│               ├── ChatScreen.svelte
│               ├── CapabilitiesScreen.svelte
│               └── ArtifactsScreen.svelte
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── build.rs
    ├── capabilities/         # Tauri capability JSON files
    ├── icons/                # platform icon set
    ├── gen/                  # generated mobile project files
    ├── macos/                # macOS-specific scaffolding
    └── src/
        ├── main.rs           # invokes browser_shell_lib::run
        ├── lib.rs            # plugin wiring + invoke_handler + setup hook
        ├── tabs.rs           # TabManager + create/close/navigate/list/save/restore
        ├── recorder.rs       # PII redaction + recorder_{start,stop,record_step,status}, capture_tab_screenshot (stub)
        ├── chat_stream.rs    # chat_stream_start/abort, ChunkPayload tagged enum, /ui/stream bridge
        ├── device_auth.rs    # DeviceAuthService, set/get/clear_device_token, from_env_or_e2e bootstrap
        ├── registration.rs   # register_capability + upload_trace_cmd
        └── telemetry.rs
```

### 7.2 Tauri 2 wiring (`src-tauri/src/lib.rs`)

Plugins (initialised in order):

1. (debug + macOS + `e2e` feature) `tauri_plugin_webdriver_automation::init()` — exposes a W3C WebDriver server for WKWebView.
2. `tauri_plugin_dialog::init()`
3. `tauri_plugin_stronghold::Builder::new(|password| blake3::hash(password.as_bytes()).as_bytes().to_vec()).build()` — derives the Stronghold key from a blake3 hash of the password.
4. `tauri_plugin_http::init()`

Managed state:

- `TabManagerState = Arc<Mutex<TabManager>>`
- `RecorderStateHandle = Arc<Mutex<RecorderState>>`
- `DeviceAuthHandle = Arc<DeviceAuthService>` (bootstrapped via `from_env_or_e2e(env::var("CONUSAI_DEVICE_TOKEN"))`)
- `StreamRegistry = Arc<Mutex<HashMap<String, JoinHandle<()>>>>`

Invoke handlers (`invoke_handler![…]`):

```
tabs::{create_tab, close_tab, navigate_tab, list_tabs, save_tabs, restore_tabs}
recorder::{recorder_start, recorder_record_step, recorder_stop, recorder_status, capture_tab_screenshot}
device_auth::{set_device_token, get_device_token, clear_device_token}
registration::upload_trace_cmd
chat_stream::{chat_stream_start, chat_stream_abort}
```

Setup hook:

1. Reads `CONUSAI_API_BASE` (default `http://localhost:8080`).
2. Emits `shell-ready` on the main webview so the frontend can pull the device token from Stronghold.
3. After 1500ms, attempts capability registration via `registration::register_capability(api_base, token)` — warns if no token.

### 7.3 RECORDER_BRIDGE_JS injection

A static JS string (in `lib.rs`) is injected into every child webview tab. It:

- Installs a singleton flag (`window.__conusai_bridge_installed__`).
- Captures `click` (anchors / buttons / `[role=button]`), `change` (form inputs), `submit` events.
- Redacts PII in the `change` handler when the field name/id matches `/password|ssn|cc-|card|cvv/i` (sets `value: null`).
- Forwards each as `invoke('recorder_record_step', { step: { kind, url, timestamp_ms, …extra } })`.

### 7.4 Native chat stream (`chat_stream.rs`)

`chat_stream_start(message, session_token, thread_id?, workspace_node_id?, attachment_ids?, api_base) -> stream_id (ulid)`:

1. Spawns a `tokio::spawn` task; stores `JoinHandle` in `StreamRegistry`.
2. POSTs to `{api_base}/ui/stream` with `X-Session-Token` and JSON body.
3. Reads `bytes_stream`, splits on `\n\n` SSE blocks, parses `data: {…}` lines.
4. Emits `ChunkPayload` (tagged enum) on `chat:chunk:<stream_id>`:
   - `Text { content }` · `ToolStart { id, name }` · `ToolResult { tool_use_id, result }` · `ThreadId { id }` · `Done` · `Error { message }`

`chat_stream_abort(stream_id)` aborts the join handle.

The frontend (`src/lib/tauri-stream.ts`) wraps this as `streamChatTauri` returning an `AsyncGenerator<ChatStreamDelta>` so it slots into `createChatStream({ streamFn })` transparently.

### 7.5 Device auth (`device_auth.rs`)

- `DeviceTokenProvider` trait — abstracts token source.
- `DeviceAuthService::from_env_or_e2e(env_token)` — uses `CONUSAI_DEVICE_TOKEN` env var or an E2E bypass when in debug.
- Commands: `set_device_token(token)`, `get_device_token() -> Option<String>`, `clear_device_token()`.
- Persistence is delegated to the Stronghold plugin from JS (frontend stores the token under a fixed record id; Rust receives only the unlocked value).

### 7.6 Tabs and recorder

`tabs.rs::TabManager`:

- `create_tab(url, title?) -> TabSummary { id, url, title }`
- `close_tab(id) -> bool`
- `navigate_tab(id, url) -> TabSummary`
- `list_tabs() -> Vec<TabSummary>`
- `save_tabs() / restore_tabs()` — persist tab state across sessions.

`recorder.rs::RecorderState`:

- `recorder_start(name)` → new SessionTrace skeleton.
- `recorder_record_step(step)` → push step (after PII filter on JS side).
- `recorder_stop() -> SessionTrace`
- `recorder_status() -> { recording: bool, step_count: usize }`
- `capture_tab_screenshot(tab_id)` — currently returns `Err("not yet implemented for this tauri version")`; future code is commented in source pending Tauri 2.2+ webview screenshot API.

### 7.7 Registration & trace upload (`registration.rs`)

- `register_capability(api_base, device_token)` — POSTs a static manifest declaring capability `trace.replay` (kind `remote_mcp`, with `replay_session` tool taking `{ trace_node_id, dry_run }`) to `/admin/capabilities/register` with `X-Device-Token`. Called in setup hook on startup.
- `upload_trace_cmd(trace: SessionTrace)` — multi-step:
  1. Serialize trace → JSON bytes.
  2. POST `/v1/files` (multipart) → `FileToken`.
  3. POST `/v1/workspaces` to create a file workspace node.
- **Contract drift**: `TraceReplayCapability.svelte` invokes `upload_trace_cmd` with `{ trace_node_id, dry_run }` instead of a `SessionTrace`. See arch.md §9 for stabilization step.

### 7.8 MobileShell composition

State (all `$state` runes):

- `user: { name, plan } | null` — restored from `localStorage["conusai_shell_user"]`.
- `nameInput`, `planInput`, `nameError` — login form.
- `workspaceNodes: WorkspaceNode[]` — declared but not currently hydrated (drawer recents partial).
- `selectedNode`, `profileSheetOpen`.

Tauri stream wiring:

```ts
const tauriStreamFn = isTauri
  ? (params) => streamChatTauri({ message, sessionToken: getSessionToken() ?? '', … })
  : undefined;
const chatStream = createChatStream(sdk, { streamFn: tauriStreamFn });
```

Auth flow (`issueSessionCookie`):

- Builds an HMAC token in-browser via `crypto.subtle.importKey('HMAC', SHA-256)` + `crypto.subtle.sign`.
- Stores in `localStorage["conusai_shell_token"]`.
- Sets `document.cookie = conusai_session=<token>; domain=<api host>; SameSite=Lax`.
- Calls `setSessionToken(token)` so subsequent SDK requests include `X-Session-Token`.

Screen routing via `screenStore.active ∈ { chat, capabilities, artifacts }`. Drawer/sheet visibility live in their own `*.svelte.ts` stores so screens can drive them without prop chains.

---

## 8. Cross-Cutting Patterns

### 8.1 SDK injection

`createConusSdk({ baseUrl, fetch, headers })` is built per app:

- **Web**: server fetches forward cookies; client fetches use the browser cookie automatically.
- **Shell**: every call attaches `X-Session-Token` from `getSessionToken()` and `X-Device-Token` from `invoke('get_device_token')`.

### 8.2 Streaming abstraction

`createChatStream` accepts a `streamFn` so the same chat surface is driven by:

- `sdk.chat.stream(params)` (web, fetch + ReadableStream)
- `streamChatTauri(params)` (shell, native bridge)

Both yield a uniform `ChatStreamDelta` discriminated union.

### 8.3 Theme + accessibility

- `THEME_SCRIPT` is inlined in `<head>` (server-rendered) to set `data-theme` before paint — avoids FOUC.
- `LiveAnnouncer.svelte` provides a global `aria-live="polite"` region for toasts and tool transitions.
- Tap targets ≥ 44px enforced by `--s-7` derived sizes in mobile parts.

### 8.4 Telemetry

Backend: `tracing` + OpenTelemetry OTLP (`opentelemetry 0.27`) → metrics scraped via `opentelemetry-prometheus`; spans correlate `tenant_id`, `plan`, `thread_id`, `tool_name`, `internal_call_id` (the Rig hook tags).

Shell: `telemetry.rs` initialises `tracing_subscriber` for in-process logs only (no OTLP shipping today).

---

## 9. Feature Inventory (Granular)

### 9.1 Backend (Rig + capabilities)

| Feature | Status | Anchor |
| --- | --- | --- |
| OpenAI-compatible `/v1/chat/completions` streaming | Implemented | `routes/chat.rs` |
| Semantic-routed `/v1/agent/completions` | Implemented | `routes/agent.rs` + `semantic_router.rs` |
| Capability registry with TOML manifests | Implemented | `capabilities/registry.rs` |
| ChainFactory (data-driven LLM chains) | Implemented | `chains/llm_chain.rs` |
| DynamicPromptCapability | Implemented | `chains/dynamic_prompt.rs` |
| WASM components (wasmtime 44 component-model) | Implemented | `capabilities/wasm_loader.rs` |
| MCP adapter | Implemented | `capabilities/mcp_adapter.rs` |
| Contract / invoice extraction (Claude vision) | Implemented | `chains/{contract,invoice}.rs` |
| Trace replay capability | Implemented | `capabilities/trace_replay.rs` |
| Local embeddings (fastembed) | Feature-gated | `indexing/local_embedding_service.rs` |
| Qdrant vector store | Implemented | `store/qdrant_vector.rs` |
| RustFS / S3 object store | Implemented | `store/rustfs_content.rs` |
| OpenAPI + Swagger UI | Implemented | `routes/mod.rs` |
| OTLP tracing + Prometheus metrics | Implemented | `agent-gateway/main.rs` |
| Job scheduler (cron) | Implemented | `jobs/scheduler.rs` |

### 9.2 Web

| Feature | Status |
| --- | --- |
| HMAC session cookie + login/logout | Implemented |
| Backend JWT adapter (env-activated) | Implemented |
| Manual scoped CSRF | Implemented |
| Workshop layout (sidebar + chat) | Implemented |
| Workspace explorer (lazy + search) | Implemented |
| Streamed chat with tool cards | Implemented |
| Multipart upload via SDK | Implemented |
| Theme switch + FOUC-free hydration | Implemented |
| Capability renderer registry context | Implemented |
| `Cmd/Ctrl+N` new session | Implemented |
| Adapter swappability in login action | Partial (login still calls `sign(...)` directly) |
| Stream parser tests path alignment | Partial (`src/tests` vs runtime path drift) |

### 9.3 Browser shell

| Feature | Status |
| --- | --- |
| Local profile onboarding + persistence | Implemented |
| HMAC cookie issuance via WebCrypto | Implemented |
| Drawer + screen stack (chat/capabilities/artifacts) | Implemented |
| Native SSE bridge (chat_stream.rs ↔ tauri-stream.ts) | Implemented |
| Stronghold token vault | Implemented |
| Capability registration on startup | Implemented |
| Native tab manager | Implemented |
| Recorder + injected DOM bridge with PII redaction | Implemented |
| Trace upload (SessionTrace → file → workspace node) | Implemented (Rust) — caller contract mismatch in UI |
| Capability list + detail sheet | Implemented |
| Artifact list rendering | Implemented |
| Artifact row open/preview action | Partial (`onClick={() => {}}`) |
| Capability invoke into chat composer | Partial (switches screen but does not prefill / dispatch) |
| Drawer recents hydration (`workspaceNodes`) | Partial |
| Tab screenshot capture | Partial (returns error pending Tauri 2.2+ API) |
| WebDriver E2E server (macOS debug) | Implemented (feature `e2e`) |
| Auto-updater | Implemented (desktop, optional `updater` feature) |

### 9.4 Shared UI

| Feature | Status |
| --- | --- |
| Foundry CSS tokens + reset | Implemented |
| Geist + Geist Mono self-hosted | Implemented |
| Paper / Forge themes with `data-theme` | Implemented |
| `prefers-reduced-motion` global guard | Implemented |
| Spring / FLIP / Stagger / Tap / ViewTransition primitives | Implemented |
| `createChatStream` with abort, inactivity guard, word RAF | Implemented |
| `WorkspaceExplorer` lazy tree + search | Implemented |
| `AgentChatComposer` with attachments | Implemented |
| `AgentChatStream` + `ToolCallCard` | Implemented |
| Capability renderer registry (Map + Svelte context) | Implemented |
| Toast + LiveAnnouncer accessibility | Implemented |
| Theme/recents/breadcrumbs/mode/featureFlags stores | Implemented |
| `CommandPalette`, `TabStrip`, `RecorderControls`, `ArtifactPreview`, `LoginPanel` | Implemented but **not mounted** in current app routes |
| `featureFlags` runtime gating | Defined but not centrally applied |

---

## 10. Stabilization Backlog

1. Align `TraceReplayCapability.svelte` payload with `upload_trace_cmd` (send full `SessionTrace`).
2. Implement artifact open/preview action in `screens/ArtifactsScreen.svelte`.
3. Hydrate `MobileShell.workspaceNodes` so `DrawerRecentChats` resolves recent threads.
4. Wire capability invoke from `CapabilitiesScreen` into chat composer (prefill + auto-send option).
5. Re-sync `e2e/shell-macos/*` selectors with current mobile shell DOM.
6. Resolve `apps/web/src/tests` path drift relative to runtime stream module location.
7. Promote `featureFlags` to a real runtime gate around `RecorderControls`, `TabStrip`, and trace replay UI.
8. Lift `apps/web` login action to use `sessionAdapter.issue(...)` so the JWT adapter actually flows.

---

## 11. Source Anchors (Granular)

- LLM core: `apps/backend/crates/agent-core/src/llm/{registry,providers/anthropic,streaming,types}.rs`
- Rig agent: `apps/backend/crates/agent-core/src/agent/{builder,runtime,hooks}.rs`
- Chains: `apps/backend/crates/agent-core/src/chains/{executor,llm_chain,dynamic_prompt,contract}.rs`
- Capability registry: `apps/backend/crates/agent-core/src/capabilities/{registry,semantic_router,trace_replay,providers/*}.rs`
- HTTP gateway: `apps/backend/crates/agent-gateway/src/{main,state,routes/*}.rs`
- Design system: `packages/ui/src/lib/{tokens.css,foundry.css,index.ts}`
- Chat state: `packages/ui/src/lib/features/createChatStream.svelte.ts`
- Capability registry (UI): `packages/ui/src/lib/capabilities/CapabilityRendererRegistry{,.svelte}.ts`
- Web session: `apps/web/src/{hooks.server,lib/server/session,lib/server/env}.ts`
- Web workshop route: `apps/web/src/routes/{+layout.server,+page.server,+page.svelte}.ts`
- Shell mobile: `apps/browser-shell/src/lib/mobile/{MobileShell.svelte,screens/*,parts/*,chrome/*,stores/*}`
- Shell native: `apps/browser-shell/src-tauri/src/{lib,chat_stream,device_auth,recorder,tabs,registration}.rs`
