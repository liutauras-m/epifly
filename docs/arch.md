# ConusAI Platform — Granular Architecture Reference

> Companion to [arch.md](arch.md). This document is a **deep, code-level reference**: full file trees, every library + version, design principles, and exact integration patterns for `apps/web`, `apps/browser-shell`, `packages/ui`, and the Rust agent runtime (with explicit Rig 0.36 usage).
>
> Audit date: 2026-05-19 · Workspace versions: backend 0.3.1 (Rust) · UI 0.6.0 · browser-shell 0.4.0 · web 0.1.0

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
apps/backend/crates/common         # error envelopes, config, telemetry, SessionTrace,
                                   #   metrics, prompt cache, path safety, http client, wasm/mcp helpers
apps/backend/crates/agent-core     # Rig pipeline, capabilities, identity (Zitadel + legacy),
                                   #   stores, embeddings, realtime hot-reload bus
apps/backend/crates/billing-core   # Lago provider, plan catalog, usage events,
                                   #   QuotaChecker, billing metrics (NEW since 2026-05-11)
apps/backend/crates/jobs           # job queue + cron scheduler + admin + JobExecutor
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
| `qdrant-client` | 1 (`serde`) | Vector DB client (1024-dim cosine, multilingual-e5-large) |
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
| `object_store` | 0.11 (`aws`) | RustFS / AWS S3 content store |
| `fastembed` | 5 (optional) | Local embeddings (`local-embeddings` feature) |
| `hex` | 0.4 | Stripe / Lago webhook signature verification |
| `colored` | 2 | Coloured CLI output |
| `uuid` / `ulid` / `chrono` | 1 / 1.1 / 0.4 | IDs + time |
| `bytes` / `futures` / `async-trait` / `bon` | 1 / 0.3 / 0.1 / 3 | Async + builder ergonomics |
| `moka` | 0.12 (`future`) | In-process cache: Zitadel token introspection (10k cap, 60s TTL) + semantic router (4096 cap, 60s TTL) |
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
| `open` | 5 | Spawn system browser for OIDC PKCE login |
| `rand` | 0.8 | PKCE verifier randomness |
| `urlencoding` | 2 | Auth-URL query encoding |
| `sha2` | workspace | PKCE S256 challenge |
| `reqwest`, `tokio`, `serde`, `serde_json`, `tracing`, `ulid`, `chrono`, `blake3`, `anyhow`, `base64` | workspace | Shared |
| `common` (path dep) | workspace | Shared error envelopes + SessionTrace types |

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
| `lucide-svelte` | ^0.477 | Icon set (used by billing/quota components) |
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
│   ├── common/                    # error envelopes, config, telemetry, SessionTrace,
│   │                              #   prompt cache, path safety, http client, audit, eval, limits
│   ├── agent-core/                # Rig pipeline + capabilities + identity + stores
│   ├── billing-core/              # Lago provider, plan catalog, quota, usage events, metrics
│   ├── agent-gateway/             # Axum binary
│   └── jobs/                      # job queue + cron scheduler + JobExecutor
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
│   ├── llm_chain.rs        # PromptChainCapability — TOML-driven LLM chain
│   ├── dynamic_prompt.rs   # DynamicPromptCapability (manifest-only prompt)
│   └── executor.rs         # run_chain shared core
│   (contract.rs, invoice.rs, extraction.rs deleted — replaced by TOML manifests in Phase 3)
├── capabilities/
│   ├── mod.rs
│   ├── manifest.rs         # ToolManifest v2 + LlmChainConfig
│   ├── card.rs             # CapabilityCard (manifest + provider + path)
│   ├── provider.rs         # CapabilityProvider, CapabilityFactory, BulkCapabilityFactory
│   ├── registry.rs         # CapabilityRegistry (with_default_factories / with_all_factories)
│   ├── namespace.rs        # NamespaceFilter
│   ├── discovery.rs        # filesystem discovery of TOML manifests + ManifestWatcher
│   ├── embedding.rs        # capability text → vector for semantic router
│   ├── executor.rs         # run_plan (single/parallel_consensus/fallback_cascade)
│   ├── semantic_router.rs  # SemanticCapabilityRouter (top-K, moka cache, blake3 keys, AttachmentHint)
│   ├── store.rs
│   ├── validator.rs
│   ├── wasm_loader.rs      # wasmtime 44 component-model loader
│   ├── mcp_adapter.rs      # MCP protocol adapter
│   ├── admin.rs
│   ├── providers/
│   │   ├── chain.rs / mcp.rs / remote_mcp.rs / wasm.rs
│   │   ├── native_storage.rs  # NativeStorageFactory + focused providers (op-dispatched)
│   │   ├── job_backed.rs      # JobBackedProvider + JobDispatch trait
│   │   ├── dynamic_prompt.rs / capability_spec.rs
│       ├── card.rs / fs.rs / cargo.rs
├── identity/                  # NEW — Zitadel OIDC + legacy provider
│   ├── mod.rs              # IdentityProvider, TenantManager, IdentityContext, AuthError
│   ├── zitadel.rs          # ZitadelProvider (openidconnect 3 + JWKS cache)
│   └── legacy.rs           # LegacyJwtProvider (HMAC tokens for dev/back-compat)
├── context/
│   ├── tenant.rs           # TenantContext + PlanTier + SubscriptionStatus + UserRole + safe_path
│   └── conversation.rs
├── memory/
│   ├── context_builder.rs
│   └── truncator.rs
├── store/
│   ├── redb_metadata.rs    # KV (postcard-encoded)
│   ├── qdrant_vector.rs    # 1024-dim cosine (multilingual-e5-large); dim verified at connect
│   ├── rustfs_content.rs   # object_store (S3 / RustFS)
│   └── marker.rs
├── vector_store/mod.rs
├── indexing/
│   ├── embedding_service.rs       # EmbeddingService trait
│   └── local_embedding_service.rs # fastembed (feature-gated); raw fastembed for query:/passage: prefix control
├── prompt/mod.rs            # PromptTemplate
├── realtime/mod.rs
└── bridge/artifact_bridge.rs
```

### 4.3 `agent-gateway` — full file structure

```
agent-gateway/src/
├── main.rs                # binary entry (also registers billing metrics + capability-spec
│                          #   realtime hot-reload listener)
├── state.rs               # AppState (registry, llm, jobs, stores, realtime, job_executor,
│                          #   capability_spec_factory, billing)
├── auth/                  # mod.rs / extractor.rs / verifier.rs — OIDC + legacy JWT verification
├── capabilities/          # mod.rs / job_backed.rs — job-backed capability providers (e.g. transcribe-video)
├── mw/                    # tower middleware
│   ├── mod.rs / admin.rs / api_key.rs / identity.rs / meter.rs / plan.rs
│   ├── rate_limit.rs / request_id.rs / router_quota.rs / tenant.rs / trace.rs
├── ui/                    # /ui/* HTML + handlers (askama templates)
│   ├── routes.rs / session.rs / handlers/
└── routes/
    ├── mod.rs             # OpenAPI assembly + SecurityAddon
    ├── auth.rs            # /v1/auth/login (legacy) + /v1/auth/zitadel/* (OIDC)
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
    ├── billing.rs              # /v1/billing/* — plans, subscription, usage
    ├── billing_webhook.rs      # /v1/billing/webhook (Lago HMAC verification)
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

`run_chain` builds Rig messages and dispatches through `LlmRegistry`:

```rust
messages.push(Message::System { content });
messages.push(Message::User {
    content: OneOrMany::one(UserContent::text(rendered_prompt)),
});
```

— resolves the `LlmRegistry` provider for `cfg.model` (alias or concrete model id) and dispatches via `CompletionProvider::complete`. JSON output is auto-parsed; non-JSON is wrapped as `{ "result": text }`.

**Domain chain removal (Phase 3):** `contract.rs`, `invoice.rs`, and `extraction.rs` were deleted. Invoice and contract extraction are now **example capabilities** declared as `kind = "chain"` TOML manifests under `apps/backend/capabilities/invoice-processing/` and `apps/backend/capabilities/contract-processing/`. All model calls go through `LlmRegistry`; no code in `agent-core` or `agent-gateway` constructs a `rig::providers::*::Client` directly (enforced by `build.rs` grep guard).

#### 4.4.6 Orchestration (`capabilities/executor.rs`)

`run_plan(steps, registry, llm, tenant)` executes a `Vec<PlanStep>` with three strategies:

| Strategy | Behaviour |
|---|---|
| `single` | Invoke one capability; return its result. |
| `parallel_consensus` | Invoke two capabilities concurrently; `llm_judge` reducer selects best result via `LlmRegistry::resolve("cheap", tenant)`. |
| `fallback_cascade` | Try primary; on error try fallback; return `{fallback: true}` if both fail. |

`OrchestrationHook` (in `agent/hooks.rs`) implements Rig's `PromptHook` interface. It detects `plan_steps` arrays in tool results, calls `run_plan`, stores results in a buffer (observer pattern — see ADR-0008), and returns `HookAction::Continue`.

#### 4.4.7 Error mapping

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

1. `McpFactory` — remote MCP servers (legacy in-process adapter).
2. `RemoteMcpFactory` — HTTP-bridged remote MCP capabilities registered via `/admin/capabilities/register`.
3. `WasmFactory` — wasmtime 44 component model (`.wasm` modules in `capabilities/`).
4. `ChainFactory::new(llm)` — instantiates `PromptChainCapability` for any manifest with `kind = "chain"` and a `[chain]` block.
5. `CapabilitySpecFactory` — declarative `kind = "capability_spec"` manifests, hot-reloadable via the realtime bus.

`NativeStorageFactory` is registered separately in `state.rs` (after the default factories) because it captures `Arc<dyn WorkspaceStore>` and `Arc<dyn WorkspaceContentStore>` which are only available at gateway startup. It handles `kind = "native"` manifests and dispatches on `config.op`.

`BuiltinFactory` was removed in Phase 4 of the capabilities refactor. `read_file` / `write_file` are now `storage-read-text` / `storage-write-text` TOML manifests; `run_cargo` is a future `compute.*` capability gated by `CONUSAI_ENABLE_DEV_TOOLS`.

`with_all_factories` additionally registers `DynamicPromptFactory`.

Each factory implements `CapabilityFactory::supports(manifest) -> bool` and `build(manifest) -> Arc<dyn CapabilityProvider>`. A `BulkCapabilityFactory` may load many cards in one boot pass (`run_bulk_load`).

### 4.7 Storage backplane

| Concern | Backend | Module |
| --- | --- | --- |
| Metadata KV | redb 2 (postcard) | `store/redb_metadata.rs` |
| Vector ANN | Qdrant 1.x (1024-dim cosine, multilingual-e5-large) | `store/qdrant_vector.rs` |
| Object content | RustFS / AWS S3 (`object_store` 0.11) | `store/rustfs_content.rs` |
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
    │   ├── CapabilityCard.svelte
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

`apps/web/src/routes/+page.svelte` calls `provideCapabilityRendererRegistry()` at the route root. `apps/browser-shell/src/routes/+page.svelte` provides the registry context at the shell layout root.

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
- `createFeatureFlags({ recorder, tabs })` — central flag gate (defined but underused at runtime today; `traceReplay` flag removed with the deleted capability).
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
    │       ├── env.ts        # BACKEND_URL + createServerFetch (forwards conusai_session cookie)
    │       ├── session.ts    # COOKIE_NAME, sign/verify, SessionAdapter, LocalHmacAdapter, BackendJwtAdapter
    │       └── oidc.ts       # NEW — Zitadel PKCE adapter (exchangeCode, revokeToken, ID-token claims)
    ├── routes/
    │   ├── +layout.server.ts # redirect → /login if !locals.user
    │   ├── +layout.svelte    # imports @conusai/ui/foundry.css, mounts ThemeProvider + ToastHost
    │   ├── +page.server.ts   # parallel load of threads, capabilities, workspace tree
    │   ├── +page.svelte      # Workshop layout
    │   ├── +error.svelte
    │   ├── login/+page.svelte + +page.server.ts (form action calls sessionAdapter.issue)
    │   ├── logout/+page.server.ts (cookie delete)
    │   ├── auth/                            # NEW — Zitadel OIDC PKCE flow
    │   │   ├── +server.ts                   # GET /auth → build auth URL, set PKCE cookies, redirect
    │   │   ├── callback/+server.ts          # /auth/callback → exchangeCode → set session cookie
    │   │   └── logout/+server.ts            # /auth/logout → revokeToken + clear cookies
    │   └── account/                         # NEW — billing & usage screens
    │       ├── +page.server.ts / +page.svelte
    │       ├── billing/+page.server.ts / +page.svelte
    │       └── usage/+page.server.ts / +page.svelte
```

### 6.2 Session machinery (`lib/server/session.ts` + `lib/server/oidc.ts`)

- Cookie: `conusai_session`, TTL `24 * 3600s`.
- `LocalHmacAdapter` (default): `sign(name, plan)` → `<base64url(payload)>.<base64url(hmacSHA256)>`.
- `BackendJwtAdapter`: activated when `BACKEND_AUTH_LOGIN_URL` is set; calls backend `/auth/login`, decodes JWT payload locally without re-verifying (gateway already verified on each call).
- **Zitadel OIDC adapter** (`oidc.ts`): activated when `AUTH_PROVIDER=zitadel`. Implements PKCE authorisation-code flow against `ZITADEL_DOMAIN`. Exposes `exchangeCode(code, verifier)`, `revokeToken(token)`, and parses `IdTokenClaims` including `urn:conusai:plan_tier` and `urn:conusai:subscription_status`. Uses two cookies: `conusai_oidc_session` (PKCE state) and `conusai_access_token`.
- `getKey()` throws in production if `UI_SESSION_KEY` is missing; dev fallback is hard-coded.
- `env.ts` exposes `BACKEND_URL` (`CONUSAI_BACKEND_URL` env, default `http://localhost:8080`) and `createServerFetch(sessionCookie)` which prefixes the backend URL and forwards the session cookie on every call.

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
│   │   └── +page.svelte      # mounts MobileShell
│   └── lib/
│       ├── sdk.ts            # createConusSdk with x-session-token + device-token headers
│       ├── tauri-stream.ts   # streamChatTauri SSE bridge
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
        ├── recorder.rs       # PII redaction + recorder_{start,stop,record_step,status}
        ├── chat_stream.rs    # chat_stream_start/abort, ChunkPayload tagged enum, /ui/stream bridge
        ├── device_auth.rs    # DeviceAuthService, set/get/clear_device_token, from_env_or_e2e bootstrap
        ├── oidc_auth.rs      # NEW — open_in_system_browser + pkce_login (PKCE flow via system browser)
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
recorder::{recorder_start, recorder_record_step, recorder_stop, recorder_status}
device_auth::{set_device_token, get_device_token, clear_device_token}
registration::upload_trace_cmd
chat_stream::{chat_stream_start, chat_stream_abort}
oidc_auth::{open_in_system_browser, pkce_login}
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

### 7.7 Registration & trace upload (`registration.rs`)

- `register_capability(api_base, device_token)` — reserved for future capability registration; called in setup hook on startup (currently no-op if no manifest configured).
- `upload_trace_cmd(trace: SessionTrace)` — multi-step:
  1. Serialize trace → JSON bytes.
  2. POST `/v1/files` (multipart) → `FileToken`.
  3. POST `/v1/workspaces` to create a file workspace node.

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
| Trace replay capability | **Deleted** (2026-05-20) — was a non-functional stub | — |
| Local embeddings (fastembed) | Feature-gated | `indexing/local_embedding_service.rs` |
| Qdrant vector store | Implemented | `store/qdrant_vector.rs` |
| RustFS / S3 object store | Implemented | `store/rustfs_content.rs` |
| OpenAPI + Swagger UI | Implemented | `routes/mod.rs` |
| OTLP tracing + Prometheus metrics | Implemented | `agent-gateway/main.rs` |
| Job scheduler (cron) | Implemented | `jobs/scheduler.rs` |
| Zitadel OIDC identity provider | Implemented | `agent-core/identity/zitadel.rs` + `routes/auth.rs` |
| Legacy JWT identity provider (HMAC dev tokens) | Implemented | `agent-core/identity/legacy.rs` |
| Lago billing provider + plan catalog | Implemented | `billing-core/{lago,catalog,provider}.rs` |
| Usage events + `QuotaChecker` middleware | Implemented | `billing-core/{quota,events}.rs` + `mw/meter.rs` |
| Billing webhook (Lago HMAC) | Implemented | `routes/billing_webhook.rs` |
| Capability-spec hot-reload via realtime bus | Implemented | `routes/realtime.rs` + `capabilities/providers/capability_spec.rs` |
| Remote-MCP capability factory (HTTP-bridged) | Implemented | `capabilities/providers/remote_mcp.rs` |
| Transcribe-video runtime capability (JobExecutor-backed) | Implemented | `agent-gateway/capabilities/job_backed.rs` (`JobBackedProvider::transcribe_video`) |

### 9.2 Web

| Feature | Status |
| --- | --- |
| HMAC session cookie + login/logout | Implemented |
| Backend JWT adapter (env-activated) | Implemented |
| Zitadel OIDC PKCE flow (`/auth`, `/auth/callback`, `/auth/logout`) | Implemented |
| Account billing page (`/account/billing`) | Implemented |
| Account usage page (`/account/usage`) | Implemented |
| Manual scoped CSRF | Implemented |
| Workshop layout (sidebar + chat) | Implemented |
| Workspace explorer (lazy + search) | Implemented |
| Streamed chat with tool cards | Implemented |
| Multipart upload via SDK | Implemented |
| Theme switch + FOUC-free hydration | Implemented |
| Capability renderer registry context | Implemented |
| `Cmd/Ctrl+N` new session | Implemented |
| Adapter swappability in login action | Partial (login still calls `sign(...)` directly) |

### 9.3 Browser shell

| Feature | Status |
| --- | --- |
| Local profile onboarding + persistence | Implemented |
| HMAC cookie issuance via WebCrypto | Implemented |
| Drawer + screen stack (chat/capabilities/artifacts) | Implemented |
| Native SSE bridge (chat_stream.rs ↔ tauri-stream.ts) | Implemented |
| Stronghold token vault | Implemented |
| OIDC PKCE login via system browser (`pkce_login`, `open_in_system_browser`) | Implemented |
| Capability registration on startup | Implemented |
| Native tab manager | Implemented |
| Recorder + injected DOM bridge with PII redaction | Implemented |
| Trace upload (SessionTrace → file → workspace node) | Implemented (Rust) — caller contract mismatch in UI |
| Capability list + detail sheet | Implemented |
| Artifact list rendering | Implemented |
| Artifact row open/preview action | Partial (`onClick={() => {}}`) |
| Capability invoke into chat composer | Partial (switches screen but does not prefill / dispatch) |
| Drawer recents hydration (`workspaceNodes`) | Partial |
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
| Billing UI components (`PlanBadge`, `PlanCard`, `QuotaBanner`, `UsageMeter`) | Implemented |
| `featureFlags` runtime gating | Defined but not centrally applied |

---

## 10. Stabilization Backlog

1. Implement artifact open/preview action in `screens/ArtifactsScreen.svelte`.
2. Hydrate `MobileShell.workspaceNodes` so `DrawerRecentChats` resolves recent threads.
3. Wire capability invoke from `CapabilitiesScreen` into chat composer (prefill + auto-send option).
4. Re-sync `e2e/shell-macos/*` selectors with current mobile shell DOM.
5. Lift `apps/web` login action to use `sessionAdapter.issue(...)` so the JWT adapter actually flows.

---

## 11. Source Anchors (Granular)

- LLM core: `apps/backend/crates/agent-core/src/llm/{registry,providers/anthropic,streaming,types}.rs`
- Rig agent: `apps/backend/crates/agent-core/src/agent/{builder,runtime,hooks}.rs`
- Chains: `apps/backend/crates/agent-core/src/chains/{executor,llm_chain,dynamic_prompt,contract}.rs`
- Capability registry: `apps/backend/crates/agent-core/src/capabilities/{registry,semantic_router,providers/*}.rs`
- Identity / Zitadel: `apps/backend/crates/agent-core/src/identity/{mod,zitadel,legacy}.rs`
- Billing: `apps/backend/crates/billing-core/src/{lib,catalog,lago,quota,events,metrics,provider,types,error}.rs`
- HTTP gateway: `apps/backend/crates/agent-gateway/src/{main,state,routes/*,mw/*,auth/*,capabilities/*}.rs`
- Billing routes: `apps/backend/crates/agent-gateway/src/routes/{billing,billing_webhook}.rs`
- Design system: `packages/ui/src/lib/{tokens.css,foundry.css,index.ts}`
- Chat state: `packages/ui/src/lib/features/createChatStream.svelte.ts`
- Capability registry (UI): `packages/ui/src/lib/capabilities/CapabilityRendererRegistry{,.svelte}.ts`
- Billing UI: `packages/ui/src/lib/components/{PlanBadge,PlanCard,QuotaBanner,UsageMeter}.svelte`
- Web session: `apps/web/src/{hooks.server,lib/server/session,lib/server/oidc,lib/server/env}.ts`
- Web auth routes: `apps/web/src/routes/auth/{+server.ts,callback/+server.ts,logout/+server.ts}`
- Web account: `apps/web/src/routes/account/{+page.{server.ts,svelte},billing/*,usage/*}`
- Web workshop route: `apps/web/src/routes/{+layout.server,+page.server,+page.svelte}.ts`
- Shell mobile: `apps/browser-shell/src/lib/mobile/{MobileShell.svelte,screens/*,parts/*,chrome/*,stores/*}`
- Shell streams: `apps/browser-shell/src/lib/{sdk.ts,tauri-stream.ts}`
- Shell native: `apps/browser-shell/src-tauri/src/{lib,chat_stream,device_auth,oidc_auth,recorder,tabs,registration}.rs`

---

## Notes

- Local object storage runs the real [RustFS](https://rustfs.com) server (Apache-2.0, `rustfs/rustfs:latest`) in SNSD mode. MinIO has been removed from the stack. See [ADR 0009 amendment](adr/0009-redb-qdrant-rustfs.md) (2026-05-19).

---

## 12. Storage Backplane — Deep Dive (2026-05-19 audit)

### 12.1 redb (embedded KV — `crates/agent-core/src/store/redb_metadata.rs`)

`RedbMetadataStore` is a single `redb::Database` file (default `/data/conusai.redb`, overridable via `REDB_PATH`) implementing **all** structured metadata stores in one process:

| Table (`TableDefinition`) | Key | Value | Encoding | Implements |
| --- | --- | --- | --- | --- |
| `threads` | `(tenant_id, thread_id)` | `Thread` | postcard | `ThreadStore::create/get` |
| `messages` | `(tenant_id, thread_id, seq u64)` | `Message` | postcard | `ThreadStore::messages` (range scan) |
| `workspace_nodes` | `(tenant_id, node_id)` | `WorkspaceNode` | **JSON** (back-compat) | `WorkspaceStore` |
| `idx_nodes_by_path` | `(tenant_id, virtual_path)` | `node_id (&str)` | utf-8 | virtual-path lookup |
| `audit_events` | `(tenant_id, ts_micros i64, event_id)` | `AuditEvent` | postcard | `AuditStore` |
| `tenant_seeded` | `tenant_id` | `u8` | byte flag | onboarding marker |

All mutations run inside one `WriteTransaction` and commit atomically; reads use snapshot `ReadTransaction`. Blocking work is wrapped in `tokio::task::spawn_blocking`. The same `Database` handle is shared with `CredentialStore` (per-tenant S3 creds, AES-256-GCM encrypted via `aes-gcm` 0.10) — redb 2 forbids two `Database` instances on one file.

Realtime bus: `tokio::sync::broadcast` (capacity 256) for workspace change events and billing SSE (`BillingEvent`). Spec hot-reload bus removed (was a no-op stub).

### 12.2 Qdrant (`crates/agent-core/src/store/qdrant_vector.rs`)

Two collections, both **1024-dim cosine (`multilingual-e5-large`), named vector `"default"`**, created idempotently on startup. Dimension is checked against the running embedding model at connect — mismatch → **drop + recreate** (resets data; prevents silent score corruption):

| Collection | Purpose | Payload schema |
| --- | --- | --- |
| `capability_embeddings` | Semantic capability routing (`SemanticCapabilityRouter`) | `capability_id`, `content`, `namespace`, `tags[]`, `metadata` |
| `content_embeddings` | Workspace content search | `tenant_id`, `owner_id`, `parent_id`, `kind`, `name`, `virtual_path`, `last_modified`, `shared_with[]`, `metadata` |

Payload indexes (created via `CreateFieldIndexCollectionBuilder` + `KeywordIndexParams`): `tenant_id` (`is_tenant=true` → Qdrant multitenant optimisation), `owner_id`, `shared_with` (array keyword). Filters built dynamically by `build_capability_filter(NamespaceFilter, tags[])`.

Test mode: `QdrantVectorStore::noop()` returns empty results without contacting the server. Connection URL via `QDRANT_URL` (default `http://qdrant:6334` — gRPC).

### 12.3 RustFS / S3 — Per-Tenant IAM (`store/tenant_storage.rs` + `store/rustfs_content.rs`)

**No raw `tenants/{id}/...` strings leak into the store layer.** All key construction is centralised in `TenantStorage` / `VirtualPath`. The factory chain is:

```
TenantStorageFactory (per-tenant clients, cached by tenant_id)
   └─ uses CredentialStore (redb, AES-256-GCM)
   └─ uses RustFsAdminClient (root creds, only for provisioning)
   └─ produces TenantStorage { object_store, bucket, prefix }
        ├─ WorkspaceStorage   ← content RW
        ├─ Presign helpers    ← upload/download URLs (real SigV4 via aws-sdk-s3 internals)
        └─ Quota accounting   ← StorageQuotaService

RustFsContentStore  → WorkspaceContentStore  (adapter for chains/memory)
TenantOnboardingService → IAM bootstrap + default workspace root + redb marker
```

Object key layout per tenant: `s3://{bucket}/{tenant_prefix}/workspace/{virtual_path}` (workspace nodes) and `…/uploads/{ulid}/{part_n}` for multipart. Server-side encryption (`RUSTFS_SSE=on`), versioning (`RUSTFS_VERSIONING=on`), bucket-level notifications (`RUSTFS_NOTIFICATIONS=on` → webhook `POST /internal/rustfs/events` on the gateway) and per-tenant quotas (`RUSTFS_QUOTAS=on`) are all bootstrapped declaratively by `rustfs-admin::bootstrap_storage` at process start.

Indexing is **event-driven** (RustFS bucket notifications → `internal/rustfs/events`), no polling watcher.

### 12.4 Postgres (shared infrastructure — NOT the agent runtime)

The **runtime agent stores no rows in Postgres**. Postgres is reserved for two infrastructure consumers, both shipped as docker services on profile `infra` / `full`:

| Consumer | Database | Purpose |
| --- | --- | --- |
| Zitadel `v2.68.0` | `zitadel` | OIDC identity, orgs, users, project roles, custom claims (`urn:conusai:plan_tier`, `urn:conusai:subscription_status`). |
| Lago `v1.30.0` (api + worker) | `lago` | Customer + subscription state, plans, usage events, invoice generation. Stripe optional via `STRIPE_SECRET_KEY`. |

The agent-gateway communicates with both **only over HTTP** (`reqwest`). `sqlx` has been fully removed from the workspace (2026-05-20).

### 12.5 Auth provider matrix (`mw/identity.rs` + `mw/tenant.rs`)

Selected by `CONUSAI_AUTH_PROVIDER` env (`legacy` default, `zitadel` opt-in):

| Provider | Token | Verification | Tenant id |
| --- | --- | --- | --- |
| `LegacyIdentityProvider` | HS256 JWT signed by `JWT_SECRET` | local `jsonwebtoken::decode` | `TenantClaims.tenant_id` |
| `ZitadelProvider` | Opaque OAuth2 access token | `POST {ZITADEL_DOMAIN}/oauth/v2/introspect` (Basic-auth using `ZITADEL_INTROSPECTION_CLIENT_ID`/`_SECRET`) | `urn:zitadel:iam:org:id` claim (falls back to `sub`) |
| `SessionUser` (`auth/verifier.rs`) | `<base64(payload)>.<base64(hmac-sha256)>` | `UI_SESSION_KEY` HMAC, constant-time compare | `CONUSAI_UI_TENANT_ID` (`dev` fallback) |
| `ApiKeyEntry` (`mw/api_key.rs`) | `X-API-Key` header | blake3 hash compared to entries in `API_KEYS` (`hash:tenant:plan,…`) | inline `tenant_id` |

Note: `ZitadelProvider` uses **introspection over reqwest** (not JWKS/openidconnect). `openidconnect` was fully removed from the workspace (2026-05-20).

### 12.6 Capability factory chain — current state

`CapabilityRegistry::with_default_factories(llm)` registers, in order: `McpFactory`, `WasmFactory`, `ChainFactory`, `CapabilitySpecFactory`. `with_all_factories` additionally registers `DynamicPromptFactory`. `NativeStorageFactory` is registered separately in `state.rs` after the executor builds (it requires `Arc<dyn WorkspaceStore>`). `BuiltinFactory` was removed in Phase 4 (2026-05-20). (`TraceReplayFactory` was deleted 2026-05-20 — the capability was a non-functional stub.) `RemoteMcpCapability` is **not** wired through a factory — it is created on demand by `POST /admin/capabilities/register` and inserted directly into the registry. `CapabilitySpecFactory` is a `BulkCapabilityFactory` invoked once at boot (`load_batch`); the hot-reload stub (`reload_one`) and capability-spec-change bus were deleted.

### 12.7 Full route surface (verified against `routes/mod.rs`)

Routes added since previous arch revision (and missing from the route tables in `project-instructions.md`):

- Public: `GET /login`, `POST /v1/auth/legacy/login`, `POST /v1/billing/webhooks`.
- Internal (network-restricted): `POST /internal/rustfs/events`.
- Protected file I/O (presign-based, no proxy download):
  `POST /v1/files/upload-url`, `GET /v1/files/download-url`,
  `POST /v1/uploads/initiate`, `POST /v1/uploads/{upload_id}/parts/{n}/presign`,
  `POST /v1/uploads/{upload_id}/complete`, `POST /v1/uploads/{upload_id}/abort`.
- Protected workspace: `POST /v1/workspaces/{id}/rename`, `POST /v1/workspaces/{id}/presign-upload`, `GET /v1/workspaces/{id}/presign-download`, `GET /v1/workspaces/nodes/{id}/versions`, `POST /v1/workspaces/nodes/{id}/restore`.
- Protected billing: `GET /v1/billing/plans`, `GET /v1/billing/subscription`, `POST /v1/billing/subscriptions`, `DELETE /v1/billing/subscription`, `POST /v1/billing/portal`, `GET /v1/billing/invoices`, `GET /v1/billing/usage`.
- Admin billing: `POST /admin/billing/credits`, `POST /admin/billing/cancel/{tenant_id}`, `GET /admin/billing/dashboard`.
- Admin tenants: `DELETE /admin/tenants/{id}`.

The legacy `GET /v1/files/{token}` UUID download shim has been removed.

### 12.8 Middleware stack (outermost → innermost on the protected router)

`CorsLayer` → `TraceLayer` → `request_id::inject_request_id` → `trace::propagate_trace` (W3C tracecontext) → `api_key::extract_api_key` → `tenant::extract_tenant` → `identity::extract_identity` (Zitadel path) → `plan::enforce_plan` (inserts `Extension<PlanLimits>`) → `meter::record_usage` (post-handler) → `RouterQuotaLayer` (route-scoped; overrides `max_tools_per_turn` / `max_invokes_per_turn` from `PlanLimits` if present, otherwise falls back to `CONUSAI_MAX_TOOLS_PER_TURN` / `CONUSAI_MAX_INVOKES_PER_TURN` env vars; enforces `QuotaChecker` daily caps with 429 + `Retry-After`).

### 12.9 Workspace members (root `Cargo.toml`) — actual

```
apps/backend/crates/common
apps/backend/crates/rustfs-admin          ← NEW, was undocumented
apps/backend/crates/agent-core
apps/backend/crates/jobs
apps/backend/crates/billing-core
apps/backend/crates/agent-gateway
apps/backend/evals
apps/browser-shell/src-tauri
```

### 12.10 Workspace dependency corrections

| Crate | Documented | Actual root `Cargo.toml` | Note |
| --- | --- | --- | --- |
| `redb` | 4 (project-instructions.md) / 2 (arch.md) | **2** | arch.md correct, project-instructions.md was wrong. |
| `rig-qdrant` | 0.2.5 (project-instructions.md) | _not present_ | Never added; ANN search goes through `qdrant-client` directly. |
| `aes-gcm` | _undocumented_ | **0.10** | Used by `CredentialStore`. |
| `openidconnect` | "Zitadel JWKS verification" | _removed_ | Deleted (2026-05-20): `ZitadelProvider` uses introspection via reqwest instead. |
| `sqlx` | TimescaleDB usage events | _removed_ | Deleted (2026-05-20): Lago owns usage event persistence over HTTP; no Postgres in agent runtime. |

---

## 13. `rustfs-admin` Crate

`apps/backend/crates/rustfs-admin` — declarative RustFS/S3 bootstrap + per-tenant IAM, presigning, quotas, and bucket notifications.

### 13.1 File tree

```
crates/rustfs-admin/src/
├── lib.rs          # RustFsAdminClient, public re-exports
├── bootstrap.rs    # BootstrapConfig, bootstrap_storage() — idempotent bucket + policy setup
├── iam.rs          # IamCreds, provision_tenant(), deprovision_tenant()
├── bucket.rs       # sanitize_bucket_name()
└── signing.rs      # presign helpers (put / get) over object_store
```

### 13.2 Public types

| Type | Description |
| --- | --- |
| `RustFsAdminClient` | Root-credential HTTP client wrapping RustFS admin API + presign |
| `BootstrapConfig` | Declarative spec: versioning, per-tenant IAM, SSE, notifications, quotas, presign TTL |
| `IamCreds` | Per-tenant `{ access_key, secret_key }` |
| `bootstrap_storage(admin, cfg)` | Idempotent bootstrap; called at gateway startup |
| `provision_tenant(admin, tenant_id)` | Create IAM user + policy scoped to `tenants/{id}/` prefix |
| `deprovision_tenant(admin, tenant_id)` | Delete IAM user + policy |

### 13.3 Environment variables

| Variable | Default | Purpose |
| --- | --- | --- |
| `RUSTFS_BOOTSTRAP` | `on` | Run `bootstrap_storage` at startup |
| `RUSTFS_VERSIONING` | `off` | Enable S3 object versioning on the workspace bucket |
| `RUSTFS_PER_TENANT_IAM` | `on` | Create per-tenant IAM users via `provision_tenant` |
| `RUSTFS_REAL_PRESIGN` | `on` | Generate real presigned URLs (off → return direct path) |
| `RUSTFS_SSE` | `off` | Enable server-side encryption on the bucket |
| `RUSTFS_NOTIFICATIONS` | `off` | Enable bucket event notifications to `RUSTFS_NOTIFICATION_WEBHOOK_URL` |
| `RUSTFS_QUOTAS` | `off` | Enable per-tenant storage quotas |
| `RUSTFS_PRESIGN_TTL_SECS` | `3600` | Presigned URL TTL |
| `RUSTFS_IAM_ENC_KEY` | — | AES-256-GCM key for encrypting IAM creds in `CredentialStore` |
| `RUSTFS_WEBHOOK_SECRET` | — | HMAC secret for validating inbound bucket notifications |
| `RUSTFS_NOTIFICATION_WEBHOOK_URL` | — | URL to receive bucket notifications (e.g. `http://agent-gateway:8080/internal/rustfs/events`) |
| `RUSTFS_ROOT_ACCESS_KEY` | — | Root access key for admin API |
| `RUSTFS_ROOT_SECRET_KEY` | — | Root secret key for admin API |

### 13.4 Bootstrap sequence

1. `bootstrap_storage` connects to `S3_ENDPOINT` with root credentials.
2. Creates `S3_BUCKET` (default `workspace`) if absent; applies versioning policy.
3. Configures SSE and notification webhook if enabled.
4. `provision_tenant` is called lazily by `TenantOnboardingService` on first user login.

---

## 14. Evals Harness

`apps/backend/evals` — binary crate (`cargo run -p evals`) for offline quality evaluation of agent pipeline outputs.

### 14.1 File tree

```
evals/src/
├── main.rs              # CLI entry (clap: run --suite <name> --dataset <jsonl> --model <id>)
├── config.rs            # EvalConfig: model, dataset path, pass threshold
├── report.rs            # EvalReport: per-run metrics + markdown / JSON output
├── runners/
│   ├── mod.rs           # run_suite() dispatch: "smoke" | "invoice" | "ocr" | "all"
│   ├── generic.rs       # Generic harness: run_suite_with_override, Scorer enum, extractors
│   ├── invoice.rs       # Invoice extraction runner (thin wrapper over generic harness)
│   └── ocr_quality.rs   # OCR quality runner (thin wrapper over generic harness)
├── suites/
│   └── smoke.jsonl      # CI smoke suite (invoice + OCR + classify, --scorer field-diff)
└── scorers/
    └── mod.rs           # InvoiceScorer (field-level F1, pass threshold 0.8)
```

### 14.2 Usage

```sh
# Run invoice extraction suite with default JSONL dataset
cargo run -p evals -- run --suite invoice --model claude-opus-4-7

# Run OCR quality suite with a custom dataset
cargo run -p evals -- run --suite ocr_quality --dataset /data/ocr-fixtures.jsonl

# List available suites
cargo run -p evals -- list
```

Requires `ANTHROPIC_API_KEY`. Uses `tracing-subscriber` for output.

---

## 15. Tauri Shell Native Modules

`apps/browser-shell/src-tauri/src/` — Rust modules compiled into the Tauri 2 desktop/mobile shell.

### 15.1 Module inventory

| Module | Purpose |
| --- | --- |
| `lib.rs` | Tauri app builder: registers all commands, plugin setup, feature-flag gating |
| `main.rs` | `main()` entry for desktop (calls `lib.rs::run()`) |
| `chat_stream.rs` | Tauri command `chat_stream` — SSE streaming proxy to `/v1/agent/completions`, emits `chat-chunk` events to the frontend |
| `device_auth.rs` | Device registration flow: generates a device keypair, calls `POST /v1/shells/{device_id}/register`, stores token in Stronghold |
| `oidc_auth.rs` | OIDC login: opens system browser to Zitadel auth URL, receives callback on a local listener, exchanges code for tokens, stores in Stronghold |
| `recorder.rs` | Implements `SessionRecorder` from `common::trace` — records `UserStep` events from the embedded web view; redacts password fields and sensitive regions |
| `registration.rs` | Platform-agnostic registration helpers shared between `device_auth.rs` and `oidc_auth.rs` |
| `tabs.rs` | Multi-tab state management for the browser-shell window (keyboard shortcuts, tab switching, close) |
| `telemetry.rs` | OTel tracer init for the Tauri process; routes spans to `OTLP_ENDPOINT` |

### 15.2 Tauri plugins (from `src-tauri/Cargo.toml`)

- `tauri-plugin-dialog` — native file/directory picker
- `tauri-plugin-stronghold` — encrypted key-value store (backs OIDC token persistence)

Feature `e2e` (macOS debug only): enables WebDriver for Playwright integration tests.

---

## 16. Identified Gaps

1. ~~**Doc/runtime drift on Zitadel.**~~ `openidconnect` deleted (2026-05-20); §12.10 updated.
2. ~~**`rustfs-admin` crate is undocumented.**~~ Documented in §13 (2026-05-20).
3. ~~**`sqlx` & `openidconnect` are dead workspace deps.**~~ Both fully removed (2026-05-20).
4. **Route tables auto-generation (Phase 10) not yet done.** `scripts/dump-routes.sh` + `--dump-routes` CLI flag + CI diff guard not yet implemented. Route tables in `project-instructions.md` §6 are manually maintained.
5. ~~**`CapabilitySpecFactory::reload_one` no-op stub + boot listener.**~~ Both deleted (2026-05-20).
6. ~~**`TraceReplayCapability` / `WorkspaceNodeTraceSource` always errors.**~~ Entire capability deleted (2026-05-20); frontend component removed.
7. ~~**`mw/plan.rs` does not clamp `max_tokens` / `max_turns`.**~~ Fixed (2026-05-20): `PlanLimits` (with `max_tools_per_turn`, `max_invokes_per_turn`) injected via `Extension<PlanLimits>`; deprecated methods removed; `RouterQuotaLayer` reads per-plan tool caps.
8. ~~**Embeddings dimension hard-coded to 768.**~~ Fixed (2026-05-20): `multilingual-e5-large` (1024-d); Qdrant dim mismatch now drops+recreates instead of hard-erroring.
9. ~~**`apps/backend/evals` not documented.**~~ Documented in §14 (2026-05-20).
10. ~~**Tauri shell modules not enumerated in §7.2.**~~ Documented in §15 (2026-05-20).
11. ~~**Dead code: `real_fs_watcher.rs`, `coco_indexer.rs` (polling path), stub prompt handlers.**~~ All deleted (2026-05-20); `WorkspaceIndexer` export removed from `agent-core`.

### Remaining plan items (not yet implemented)

- **Phase 10** — `scripts/dump-routes.sh` + `--dump-routes` binary flag + CI diff guard (`make verify-routes-doc`).
- **CI hardening** — `cargo machete` blocking step; `clippy::todo` / `clippy::unimplemented` = deny.
- **`spec_reload_total`** Prometheus counter (requires implementing capability hot-reload, not just deleting it).
- **`scripts/reindex.sh`** — smooth re-embed migration for operators upgrading from old embedding models.
- **`daily_quota`** field in `PlanLimits` (currently handled by `QuotaChecker` separately; not blocking).

