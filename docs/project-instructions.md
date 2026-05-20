You are an expert AI-agents Rust developer. Build maintainable, extensible agent software — SRP, clean code, community-canonical naming.

**Rules:** no unnecessary features; rename boldly; newest idiomatic practices; reusable code; AI-hours + token cost; challenge every decision.

**Key resource:** https://docs.rig.rs · See [arch.md](arch.md) for the full architecture audit.

---

## Canonical Names

- `Agent`, `AgentBuilder` — core runtime + builder.
- `CompletionProvider`, `LlmRegistry`, `LlmBinding` — provider-agnostic model surface (Rig `Completion*`).
- `CapabilityProvider`/`Factory`/`Registry`/`Card`/`Admin` — replaces "Tool*"; richer (prompts, chains, memory, permissions).
- `PromptChainCapability`, `DynamicPromptCapability`, `SemanticCapabilityRouter`.
- `IdentityProvider`/`TenantManager`/`Identity|TenantContext` — pluggable (legacy JWT / Zitadel).
- `PlanLimits` — resolved once per request via `PlanTier::limits()`, injected as `Extension<PlanLimits>` by `enforce_plan` middleware. Fields: `max_tokens`, `max_turns`, `rate_limit_rpm`, `max_tools_per_turn`, `max_invokes_per_turn`. Handlers must NOT call deprecated `max_tokens()`/`max_turns()`/`rate_limit_rpm()`. `RouterQuotaLayer` reads `max_tools_per_turn`/`max_invokes_per_turn` from `PlanLimits` when present.
- `BillingProvider`, `QuotaChecker`, `PlanCatalog`, `UsageEvent` — pluggable billing (Lago).
- `TenantStorage`/`Factory`, `VirtualPath`, `CredentialStore`, `RustFsAdminClient` — per-tenant S3 abstraction.
- `RedbMetadataStore`, `QdrantVectorStore`, `RustFsContentStore` — the three runtime stores; **no Postgres in the agent runtime**. Qdrant dim mismatch at boot → collection dropped and recreated (no hard error).
- `ArtifactBridge` — materialises tool artifacts into workspace + object storage.

---

## 1. Monorepo Layout

```
conusai-platform/
├─ apps/{backend, web, browser-shell}     ← Rust workspace; SvelteKit (Node); Tauri 2 + iOS/Android (SvelteKit static)
├─ packages/{ui, sdk, types}              ← @conusai/* — Svelte 5 DS, typed HTTP client, OpenAPI types
├─ services/current-time/                 ← sample self-registering MCP capability
├─ docker-compose.yml                     ← profiles: infra | full | observability | marker | web
└─ docs/ (arch.md canonical) · start.sh · Makefile · justfile
```

pnpm workspace: `apps/web`, `apps/browser-shell`, `packages/*`. Web pulls in `@conusai/{ui,sdk,types}`; browser-shell adds `@tauri-apps/{api,plugin-dialog,plugin-stronghold}`.

## 2. Backend (`apps/backend/`)

### 2.1 Cargo Workspace Members

```
crates/common          ← shared types, error envelopes, prompt template, telemetry
crates/rustfs-admin    ← bootstrap, IAM, presign, quotas, bucket notifications
crates/agent-core      ← Agent runtime, capabilities, stores, identity, llm, chains
crates/jobs            ← scheduler, registry, executor, admin
crates/billing-core    ← Lago provider, plan catalog, quota checker, usage events
crates/agent-gateway   ← axum HTTP, routes, middleware, AppState, main.rs
evals                  ← runners + scorers
apps/browser-shell/src-tauri  ← Tauri 2 native shell
```

### 2.2 Toolchain

Rust edition **2024**, rust-version **1.95**, resolver `"3"`, workspace version `0.3.1`. WASM `wasm32-wasip1`. `rust-toolchain.toml`: `rustfmt`, `clippy`, `rust-src`, `rust-analyzer`. Release: `opt-level=3`, `lto="thin"`, `codegen-units=1`, `strip="symbols"`.

### 2.3 Key Workspace Dependencies

LLM: `rig-core` 0.36 (native `CompletionModel::stream()`). Storage: `redb` 2 + `postcard` 1; `qdrant-client` 1 (default-features=false, +serde, **1024-d cosine** via `multilingual-e5-large`); `object_store` 0.11 (+aws). HTTP: `axum` 0.8 (ws, multipart), `tower-http` 0.6, `reqwest` 0.13. Crypto: `jsonwebtoken` 9, `blake3` 1, `hmac` 0.12, `aes-gcm` **0.10**. WASM: `wasmtime` 44 (component-model). Observability: `tracing` + `opentelemetry` 0.27 (+otlp/prometheus), `tracing-opentelemetry` 0.28. Misc: `utoipa` 5 + `utoipa-swagger-ui` 9, `tokio-cron-scheduler` 0.13, `moka` 0.12 (token cache + semantic router cache), `figment` 0.10, `bon` 3, `ulid` 1.1, `hex` 0.4, `fastembed` 5 (feature `local-embeddings`; default model `multilingual-e5-large`). **No `rig-qdrant`, no runtime `sqlx`, no `openidconnect`.**

## 3. Storage Backplane

- **`RedbMetadataStore`** — embedded `redb` 2 file at `REDB_PATH` (default `/data/conusai.redb`). Tables: `threads`, `messages` (range-scanned by `(tenant,thread,seq)`), `workspace_nodes` (JSON), `idx_nodes_by_path`, `audit_events`, `tenant_seeded` (postcard except workspace nodes).
- **`CredentialStore`** — same redb file; per-tenant S3 creds encrypted with **AES-256-GCM** (`RUSTFS_IAM_ENC_KEY`).
- **`QdrantVectorStore`** — `QDRANT_URL` (gRPC `:6334`). Collections `capability_embeddings` + `content_embeddings`, **1024-d cosine** (multilingual-e5-large), named vector `"default"`. Dimension mismatch at connect → hard error (no silent failure). Payload keyword indexes on `tenant_id` (`is_tenant=true`), `owner_id`, `shared_with`.
- **`RustFsContentStore` / `TenantStorage`** — `object_store` 0.11 over RustFS/S3 (`S3_ENDPOINT`, `S3_BUCKET`); per-tenant IAM, presign, multipart, SSE, versioning, quotas.
- **Postgres 17** (docker `infra` profile) — **only** for Zitadel + Lago databases; no agent runtime rows.

## 4. Identity, Tenants, Billing

- `IdentityManager` selects `LegacyIdentityProvider` (HS256 JWT via `JWT_SECRET`) or `ZitadelProvider` (REST `/oauth/v2/introspect`, mgmt API) via `CONUSAI_AUTH_PROVIDER` (`legacy`|`zitadel`). Zitadel claims: `urn:zitadel:iam:org:id` → tenant; `urn:zitadel:iam:org:project:roles` → role; `urn:conusai:{plan_tier,subscription_status}` → plan gating.
- `SessionUser` cookie (`conusai_session`) or `X-Session-Token` header — HMAC-SHA256 (`UI_SESSION_KEY`). `X-API-Key` matched against blake3 hashes from `API_KEYS=hash:tenant:plan,…`.
- `billing-core`: `LagoProvider` (1 s flush loop) + `PlanCatalog` + `QuotaChecker` (daily) + `UsageEvent`. Stripe-via-Lago. Env: `LAGO_API_URL`, `LAGO_API_KEY`, `LAGO_WEBHOOK_SIGNATURE_SECRET`.

## 5. Capability Stack

- Factories on `CapabilityRegistry`: `Mcp`, `Wasm`, `Chain`, `Builtin` (+ `DynamicPrompt` via `with_all_factories`). Remote MCP capabilities self-register via `POST /admin/capabilities/register`. **`TraceReplayCapability` deleted** — was always a stub that errored on every call.
- `CapabilitySpecFactory` (`BulkCapabilityFactory`) `load_batch`-materialises enabled specs at boot; strategies: `dynamic_prompt`, `prompt` (chain), `wasm`, `native`, `remote_mcp`.
- `SemanticCapabilityRouter`: blake3-keyed `moka::future::Cache` (max 4096, TTL 60 s), defaults **`top_k=20`**, **`max_distance=0.38`**. Returns `Vec<Box<dyn rig::tool::ToolDyn>>` to `AgentBuilder`.
- `ArtifactBridge` persists tool artifacts to workspace + object storage post-tool and emits realtime events.

## 6. HTTP Surface (gateway = `agent-gateway`)

Four router groups in `routes/mod.rs`: `public_router`, `protected_router(quota)`, `admin_router`, `internal_router` (firewall-only).

- **Public:** `GET /health` · `GET /login` · `POST /v1/auth/login` (+ `/legacy/login`) · `POST /v1/billing/webhooks` (Lago) · `POST /admin/capabilities/register` (self-registration) · `GET /openapi.json` · `GET /docs` (Swagger) · `GET /metrics` (Prometheus).
- **Protected (tenant mw + `RouterQuotaLayer`):**
  - Agent/chat/MCP: `POST /v1/{chat,agent}/completions`; `GET /v1/capabilities[/search]`; `POST /mcp`.
  - Files (presign-only, no proxy): `POST /v1/files/upload-url`, `GET /v1/files/download-url`; multipart `POST /v1/uploads/{initiate,{id}/parts/{n}/presign,{id}/complete,{id}/abort}`.
  - Workspaces: CRUD on `/v1/workspaces[/{id}[/content]]` + actions `{move,rename,share,unshare,presign-upload,presign-download}`; node versions: `GET /v1/workspaces/nodes/{id}/versions`, `POST .../restore`.
  - Tasks/threads/realtime: `GET /v1/tasks[/{id}[/sse]]`; `GET /v1/threads/{id}/messages`; `GET /api/realtime/workspace`; `GET /v1/shells/{device_id}/control`; `GET /v1/audit`.
  - Billing: `GET /v1/billing/{plans,subscription,invoices,usage}`; `POST /v1/billing/{subscriptions,portal}`; `DELETE /v1/billing/subscription`.
- **Super-admin (`require_super_admin_jwt`):** `/admin/{capabilities,jobs,tasks,devices,billing}*`; `DELETE /admin/tenants/{id}`.
- **Internal:** `POST /internal/rustfs/events` — RustFS bucket notifications drive event-driven workspace indexing.

**CORS:** `build_cors()` reads `WEB_ORIGIN` (default `localhost:3000`, `5173`, `tauri://localhost`, `https://tauri.localhost`); allows `Authorization`, `Content-Type`, `X-Tenant-Id`, `X-API-Key`, `X-Session-Token`; exposes `X-Request-Id`; `allow_credentials=true`. Never `permissive()`.

## 7. Middleware Order (`agent-gateway/src/mw/`)

`CorsLayer` → `TraceLayer` → `request_id` → `trace` (W3C) → `api_key` → `tenant` → `identity` (`ResolvedIdentity`) → `plan` → `meter` (`AgentTurnStats` → `BillingProvider::report_usage` + `QuotaChecker::record`) → `RouterQuotaLayer` (route-scoped; daily quota → 429 with `Retry-After` + `{"upgrade_url":…}`).

## 8. `AppState` (`agent-gateway/src/state.rs`)

`Agent`, `CapabilityRegistry`, `SemanticCapabilityRouter`, the three stores (redb/qdrant/rustfs), `rustfs_admin`, `cred_store`, `tenant_storage`, `onboarding`, `storage_quota`, `rustfs_metrics`, `onboarding_guards`, `device_tokens`, `identity` (`IdentityManager`), `billing: Option<Arc<dyn BillingProvider>>`, `quota: Option<Arc<QuotaChecker>>`, `plan_catalog`.

## 9. Environment Matrix

- **Auth:** `JWT_SECRET`, `UI_SESSION_KEY`, `API_KEYS`, `CONUSAI_AUTH_PROVIDER`, `CONUSAI_UI_TENANT_ID`, `DEV_PASSWORD`, `PLATFORM_ADMIN_TOKEN`.
- **Zitadel:** `ZITADEL_{DOMAIN,AUDIENCE,INTROSPECTION_CLIENT_ID,INTROSPECTION_CLIENT_SECRET,MGMT_PAT,MASTERKEY}`.
- **Billing:** `LAGO_API_URL`, `LAGO_API_KEY`, `LAGO_WEBHOOK_SIGNATURE_SECRET`, `LAGO_*` (enc + RSA), `STRIPE_SECRET_KEY`.
- **Storage:** `REDB_PATH`, `QDRANT_URL`, `S3_ENDPOINT`, `S3_BUCKET`, `AWS_ACCESS_KEY_ID`/`_SECRET`.
- **RustFS bootstrap:** `RUSTFS_{BOOTSTRAP,VERSIONING,PER_TENANT_IAM,REAL_PRESIGN,SSE,NOTIFICATIONS,QUOTAS,PRESIGN_TTL_SECS,IAM_ENC_KEY,WEBHOOK_SECRET,NOTIFICATION_WEBHOOK_URL,ROOT_ACCESS_KEY,ROOT_SECRET_KEY}`.
- **LLM:** `ANTHROPIC_API_KEY`, `EMBEDDING_BACKEND` (`local` only; default), `EMBEDDING_LOCAL_MODEL` (`multilingual-e5-large`|`bge-m3`|`nomic-embed-text-v1.5`|`all-minilm-l6-v2`).
- **Quotas/Web/Telemetry:** `CONUSAI_MAX_TOOLS_PER_TURN`, `CONUSAI_MAX_INVOKES_PER_TURN`, `WEB_ORIGIN`, `CONUSAI_BACKEND_URL`, `CONUSAI_FEATURE_BROWSER_SHELL`, `OTLP_ENDPOINT`, `RUST_LOG`.

## 10. Docker Compose Service Catalog

- **infra/full:** `postgres:17-alpine` (5432), `redis:7-alpine` (6379), `zitadel:v2.68.0` (8085→8080), `getlago/api:v1.30.0` api (3010→3000) + worker.
- **default:** `qdrant:v1.17.0` (6333/6334), `rustfs-perms` one-shot, `rustfs:latest` (9000/9001), `agent-gateway` (8080), `services/current-time` (8082).
- **marker:** `marker:latest` (8081→8080). **web/full:** `node:22-slim` running `apps/web/build` (3000).
- **observability:** `jaeger:1.58` (16686, 14317), `otel-collector-contrib:0.123.0` (4317, 4318).

Volumes: `postgres_data`, `qdrant_data`, `rustfs_data`, `redb_data`.

## 11. Frontend

- **`@conusai/ui`** (Svelte 5, `sideEffects: ["**/*.css"]`) — `tokens.css`, `foundry.css`; components `AppShell, CapabilityCard, PlanBadge, PlanCard, QuotaBanner, ThemeProvider/Script/Switcher, ToastHost, UsageMeter, WorkspaceTree`; features `AgentChatComposer, AgentChatStream, ToolCallCard, WorkspaceExplorer` + `createChatStream.svelte.ts`; plus `capabilities/`, `stores/`, `motion/`, `utils/`.
- **`apps/web`** — SvelteKit (Node adapter); uses `@conusai/{ui,sdk,types}`; `UI_SESSION_KEY`-signed cookies; OIDC callback under `src/routes/auth/*`.
- **`apps/browser-shell`** — SvelteKit (static) + Tauri 2 (`@tauri-apps/{api,plugin-dialog,plugin-stronghold}`); mobile shell `src/lib/mobile/*`; native modules in `src-tauri/src/*.rs` (chat_stream, device_auth, oidc_auth, recorder, registration, tabs, telemetry); debug-only WebDriver via feature `e2e` (macOS).

## 12. Architecture Decisions

ADRs in `docs/adr/`: 0003 (Unified Postgres + CocoIndex — *superseded*), 0004 (Semantic Router & Dynamic Prompts), 006 (Tauri Browser Shell), 007 (Capability Module Rename), 008 (Multi-Platform Shell), 0009 (redb + Qdrant + RustFS), 012 (Zitadel + Lago).

> Known drift: `sqlx` declared-but-unused at runtime (Lago owns usage HTTP). All other previously noted drift items resolved (2026-05-20): `openidconnect` deleted, reload_one stub deleted, dim fixed to 1024, `rustfs-admin`/`evals`/Tauri modules documented in arch.md §13–15, `PlanLimits` tool caps wired to `RouterQuotaLayer`, Qdrant dim mismatch now resets collections.
