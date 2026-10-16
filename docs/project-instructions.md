You are an expert AI-agents Rust developer. Build maintainable, extensible agent software — SRP, clean code, community-canonical naming.

**Rules:** no unnecessary features; rename boldly; newest idiomatic practices; reusable code; AI-hours + token cost; challenge every decision.

**Key resource:** https://docs.rig.rs · See [arch.md](arch.md) for the full architecture audit.

---

## Canonical Names

- `Agent`, `AgentBuilder` — core runtime + builder.
- `CompletionProvider`, `LlmRegistry`, `LlmBinding` — provider-agnostic model surface (Rig `Completion*`).
- `CapabilityProvider`/`Factory`/`Registry`/`Card`/`Admin` — replaces "Tool*"; richer (prompts, chains, memory, permissions).
- `PromptChainCapability`, `DynamicPromptCapability`, `SemanticCapabilityRouter`.
- `TokenVerifier`/`ZitadelVerifier`/`Identity`/`TenantContext` — Zitadel access-token verification; `auth::authenticate` is the one place a `TenantContext` is built.
- `PlanResolver` — the tenant's plan tier from its Lago subscription (cached, Free by default); never from a token or a request.
- `PlanLimits` — resolved once per request via `PlanTier::limits()`, injected as `Extension<PlanLimits>` by `enforce_plan` middleware. Fields: `max_tokens`, `max_turns`, `rate_limit_rpm`, `max_tools_per_turn`, `max_invokes_per_turn`. `RouterQuotaLayer` reads `max_tools_per_turn`/`max_invokes_per_turn` from `PlanLimits` when present.
- `BillingProvider`, `QuotaChecker`, `PlanCatalog`, `UsageEvent` — pluggable billing (Lago).
- `TenantStorage`/`Factory`, `VirtualPath`, `CredentialStore`, `RustFsAdminClient` — per-tenant S3 abstraction.
- `RedbMetadataStore`, `QdrantVectorStore`, `RustFsContentStore` — the three runtime stores; **no Postgres in the agent runtime**. Qdrant dimension mismatch at boot → the gateway refuses to start (no silent drop).
- `ArtifactBridge` — materialises tool artifacts into workspace + object storage.

---

## 1. Monorepo Layout

```
conusai-platform/
├─ apps/{backend, web, browser-shell}     ← Rust workspace; SvelteKit (Node); Tauri 2 + iOS/Android (SvelteKit static)
├─ packages/{ui, sdk, types}              ← @conusai/* — Svelte 5 DS, typed HTTP client, OpenAPI types
├─ services/current-time/                 ← sample self-registering MCP capability
├─ docker-compose.yml                     ← what ConusAI Cloud deploys (gateway, web, current-time, zitadel, lago-*)
├─ docker-compose.local.yml               ← local only: Postgres, Redis, Qdrant, RustFS + host ports (`just up`)
├─ conusai.json · .env.example · scripts/generate-env.sh
└─ docs/ (arch.md canonical; deploy-conusai-cloud.md) · Makefile · justfile
```

pnpm workspace: `apps/web`, `apps/browser-shell`, `packages/*`. Web pulls in `@conusai/{ui,sdk,types}`; browser-shell adds `@tauri-apps/{api,plugin-dialog}` (Stronghold is used from Rust only).

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
xtask                  ← cargo xtask (capability lint, …)
apps/browser-shell/src-tauri  ← Tauri 2 native shell (excluded from the Cargo workspace)
```

### 2.2 Toolchain

Rust edition **2024**, rust-version **1.99**, resolver `"3"`, workspace version `0.3.1`. `rust-toolchain.toml`: channel `1.99.1`, components `rustfmt`, `clippy`. Release: `opt-level=3`, `lto="thin"`, `codegen-units=1`, `strip="symbols"`.

### 2.3 Key Workspace Dependencies

LLM: `rig-core` 0.36 (native `CompletionModel::stream()`). Storage: `redb` 2 + `postcard` 1; `qdrant-client` 1 (default-features=false, +serde, **1024-d cosine** via `multilingual-e5-large`); `object_store` 0.11 (+aws). HTTP: `axum` 0.8 (ws, multipart), `tower-http` 0.6, `reqwest` 0.13. Crypto: `jsonwebtoken` 11 (RS256 via JWKS), `blake3` 1, `hmac` 0.12, `aes-gcm` **0.10**, `argon2` 0.5. WASM: `wasmtime` 49. Observability: `tracing` + `opentelemetry` 0.33 (OTLP **HTTP/protobuf** only, + prometheus), `tracing-opentelemetry` 0.34. Misc: `utoipa` 5 (OpenAPI document only, no Swagger UI), `tokio-cron-scheduler` 0.13, `moka` 0.12 (JWKS/introspection/plan caches + semantic router cache), `bon` 3, `ulid` 1.1, `hex` 0.4, `fastembed` 5 (feature `local-embeddings`; default model `multilingual-e5-large`). **No `rig-qdrant`, no runtime `sqlx`, no `openidconnect`.**

## 3. Storage Backplane

- **`RedbMetadataStore`** — embedded `redb` 2 file at `REDB_PATH` (default `/data/conusai.redb`; the image sets `/data/epifly.redb`). Tables: `threads`, `messages` (range-scanned by `(tenant,thread,seq)`), `workspace_nodes` (JSON), `idx_nodes_by_path`, `audit_events`, `tenant_seeded` (postcard except workspace nodes).
- **`CredentialStore`** — same redb file; per-tenant S3 creds encrypted with **AES-256-GCM** (`RUSTFS_IAM_ENC_KEY`).
- **`QdrantVectorStore`** — `QDRANT_GRPC_URL` + `QDRANT_API_KEY` (gRPC `:6334`, injected by the linked Qdrant service). Collections `capability_embeddings_d{dims}` + `content_embeddings_d{dims}` (`_d1024` for multilingual-e5-large), cosine, named vector `"default"`. Dimension mismatch at connect → hard error (no silent failure). Payload keyword indexes on `tenant_id` (`is_tenant=true`), `owner_id`, `shared_with`.
- **`RustFsContentStore` / `TenantStorage`** — `object_store` 0.11 over S3 (`AWS_ENDPOINT_URL`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `S3_BUCKET`, injected by the linked S3 service; RustFS locally); per-tenant IAM, presign, multipart, SSE, versioning, quotas.
- **Postgres** (linked managed service; `postgres:18.6-alpine` locally) — **only** for the Zitadel + Lago databases; no agent runtime rows.

## 4. Identity, Tenants, Billing

- Every protected route takes `Authorization: Bearer <token>` only (`agent-gateway/src/auth.rs`); no cookies, session headers, tenant headers or credential-less modes.
  - **Zitadel access token** — `ZitadelVerifier` (`agent-core/src/identity/zitadel.rs`): JWTs verified against the issuer's JWKS (RS256 only, `iss` = `ZITADEL_ISSUER`, `aud` ∋ `ZITADEL_AUDIENCE`, `exp`/`nbf`); opaque tokens via introspection only when `ZITADEL_INTROSPECTION_CLIENT_ID`/`_SECRET` are set. Tenant = `urn:zitadel:iam:user:resourceowner:id` (the user's organization; a token without it is rejected); role from `urn:zitadel:iam:org:project:roles` (`super_admin` → `/admin/*`, `admin`).
  - **API key** — BLAKE3 hashes in `API_KEYS=<blake3-hex>:<tenant>:<label>,…`; `agent-gateway --hash-api-key` reads a key on stdin and prints its hash.
- Plan tier: `PlanResolver` (`agent-gateway/src/plans.rs`) from the tenant's Lago subscription (`pro` → Pro, `team`/`enterprise` → Enterprise, otherwise Free), cached 5 min, invalidated by subscription webhooks. The chat model is chosen from the plan's alias allowlist (`PlanTier::allowed_aliases`).
- `billing-core`: `LagoProvider` (1 s flush loop) + `PlanCatalog` + `QuotaChecker` (daily) + `UsageEvent`. Billing is on when `LAGO_API_KEY` is set; then `LAGO_API_URL` and `LAGO_WEBHOOK_SECRET` (Lago's organization HMAC key) are required.

## 5. Capability Stack

- Factories on `CapabilityRegistry`: `McpFactory`, `WasmFactory`, `ChainFactory` (+ `DynamicPromptFactory` via `with_all_factories`); the gateway adds `NativeStorageFactory`. Manifest kinds: `mcp`, `wasm`, `chain`, `docker`, `native`, `dynamic_prompt`, `remote_mcp`. Remote MCP capabilities self-register via `POST /admin/capabilities/register` (`PLATFORM_ADMIN_TOKEN`); their endpoint host must be in `CONUSAI_MCP_ALLOWED_HOSTS`.
- `SemanticCapabilityRouter`: blake3-keyed `moka::future::Cache` (max 4096, TTL 60 s), defaults **`top_k=20`**, **`max_distance=0.38`**. Returns `Vec<Box<dyn rig::tool::ToolDyn>>` to `AgentBuilder`.
- `ArtifactBridge` persists tool artifacts to workspace + object storage post-tool and emits realtime events.

## 6. HTTP Surface (gateway = `agent-gateway`)

Four router groups in `routes/mod.rs`: `public_router`, `protected_router(quota)`, `admin_router`, `internal_router` (firewall-only).

- **Public:** `GET /health` · `GET /healthz/embeddings` · `POST /v1/billing/webhooks` (Lago, HMAC) · `POST /admin/capabilities/register` (self-registration, `PLATFORM_ADMIN_TOKEN`) · `GET /openapi.json` (also `agent-gateway --dump-openapi`) · `GET /metrics` (Prometheus, `Authorization: Bearer <PLATFORM_ADMIN_TOKEN>`).
- **Protected (`auth::authenticate` → `enforce_plan` → `record_usage`; `RouterQuotaLayer` on the two completion routes):**
  - Agent/chat/MCP: `POST /v1/{chat,agent}/completions`; `GET /v1/capabilities[/search]`; `POST /mcp`.
  - Files (presign-only, no proxy): `POST /v1/files/upload-url`, `GET /v1/files/download-url`; multipart `POST /v1/uploads/{initiate,{id}/parts/{n}/presign,{id}/complete,{id}/abort}`.
  - Workspaces: CRUD on `/v1/workspaces[/{id}[/content]]` + actions `{move,rename,share,unshare,presign-upload,presign-download}`; node versions: `GET /v1/workspaces/nodes/{id}/versions`, `POST .../restore`.
  - Tasks/threads/realtime: `GET /v1/tasks[/{id}[/sse]]`; `GET /v1/threads[/{id}/messages]`; `GET /api/realtime/workspace`; `GET /v1/audit`.
  - UI streaming: `POST /ui/stream`, `POST /ui/upload`, `GET /ui/files/download`.
  - Billing: `GET /v1/billing/{plans,subscription,invoices,usage}`; `POST /v1/billing/{subscriptions,portal}`; `DELETE /v1/billing/subscription`.
- **Super-admin (`auth::require_super_admin`):** `/admin/{capabilities,jobs,tasks,billing}*`; `DELETE /admin/tenants/{id}`.
- **Internal (mounted only when `RUSTFS_NOTIFICATION_WEBHOOK_URL` + `RUSTFS_WEBHOOK_SECRET` are set):** `POST /internal/rustfs/events` — bucket notifications drive event-driven workspace indexing.

`agent-gateway --dump-routes` prints the route table (`ROUTE_TABLE` in `routes/mod.rs`).

**CORS:** `build_cors()` allows the required `WEB_ORIGIN` list plus the native shell origins `tauri://localhost` and `http://tauri.localhost`; allows `Authorization`, `Content-Type`, `traceparent`; exposes `X-Request-Id`; no credentialed CORS (no cookies). Never `permissive()`.

## 7. Middleware Order (`agent-gateway/src/mw/`)

`CorsLayer` → `TraceLayer` → `request_id` → `trace` (W3C) → `auth::authenticate` (`ResolvedTenant`) → `plan` (`Extension<PlanLimits>`) → `meter` (`AgentTurnStats` → `BillingProvider::report_usage` + `QuotaChecker::record`) → `RouterQuotaLayer` (route-scoped; daily quota → 429 with `Retry-After` + `{"upgrade_url":…}` from `BILLING_RETURN_URL`, default `/account/billing`).

## 8. `AppState` (`agent-gateway/src/state.rs`)

`Agent`, `CapabilityRegistry`, `SemanticCapabilityRouter`, the three stores (redb/qdrant/rustfs), `rustfs_admin`, `cred_store`, `tenant_storage`, `onboarding`, `storage_quota`, `rustfs_metrics`, `onboarding_guards`, `verifier` (`Arc<dyn TokenVerifier>`), `api_keys`, `plans` (`PlanResolver`), `platform_admin_token`, `billing: Option<Arc<dyn BillingProvider>>`, `quota: Option<Arc<QuotaChecker>>`, `plan_catalog`.

## 9. Environment Matrix

The gateway's whole contract is `GatewayConfig::from_env` in `agent-gateway/src/config.rs` (built on `common/src/env.rs`): it is validated at start, and a missing or invalid variable exits with code 2 naming every offender. There are no development fallbacks. The deployment table (which service, how to generate, what must never be regenerated) is in [deploy-conusai-cloud.md](deploy-conusai-cloud.md).

- **Auth:** `ZITADEL_ISSUER`, `ZITADEL_AUDIENCE`, optional `ZITADEL_INTROSPECTION_CLIENT_ID`/`_SECRET`, optional `API_KEYS`, `PLATFORM_ADMIN_TOKEN` (≥ 32 chars), `WEB_ORIGIN`.
- **Billing:** `LAGO_API_KEY` (optional; enables billing), then `LAGO_API_URL`, `LAGO_WEBHOOK_SECRET`; optional `CONUSAI_PLAN_CATALOG_PATH`, `BILLING_RETURN_URL`.
- **Storage:** `REDB_PATH`, `QDRANT_GRPC_URL`, `QDRANT_API_KEY`, `AWS_ENDPOINT_URL`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `S3_BUCKET`, `RUSTFS_IAM_ENC_KEY` (base64 of 32 bytes).
- **RustFS bootstrap:** `RUSTFS_{BOOTSTRAP,SSE,VERSIONING,QUOTAS}` (`on`/`off`, default `on`), `RUSTFS_PRESIGN_TTL_SECS`, `RUSTFS_PUBLIC_BASE_URL`, `RUSTFS_KEY_ROTATION_DAYS`, `RUSTFS_NOTIFICATION_WEBHOOK_URL` + `RUSTFS_WEBHOOK_SECRET` (both or neither).
- **Capabilities/LLM:** `CONUSAI_CAPABILITIES_DIR`, `CONUSAI_MCP_ALLOWED_HOSTS`, `ANTHROPIC_API_KEY`, `EMBEDDING_CACHE_DIR`, `EMBEDDING_LOCAL_MODEL` (`multilingual-e5-large` default, `bge-small-en-v1.5`, `bge-m3`, `nomic-embed-text-v1.5`, `all-minilm-l6-v2`), `EMBEDDING_MAX_BATCH`, `SEMANTIC_ROUTER_TOP_K`; `OPENAI_API_KEY` only for the video-transcription job.
- **Quotas/Telemetry:** `CONUSAI_MAX_TOOLS_PER_TURN`, `CONUSAI_MAX_INVOKES_PER_TURN`, `CONUSAI_ROUTER_MIN_CONFIDENCE`, `AUDIT_RETENTION_DAYS`, `PORT`, `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_HEADERS`, `OTEL_SERVICE_NAME`, `RUST_LOG`.

## 10. Docker Compose Service Catalog

- **`docker-compose.yml`** (deployed by ConusAI Cloud): `gateway` (8080, `apps/backend/Dockerfile`), `web` (3000, `apps/web/Dockerfile`), `current-time` (8082), `zitadel` (`ghcr.io/zitadel/zitadel:v4.19.5`, 8080), `lago-migrate` (job) + `lago-api` (3000) + `lago-worker` + `lago-clock` (`getlago/api:v1.55.0`). Postgres, Redis, Qdrant and S3 are linked managed services. Public ports are declared in `conusai.json`.
- **`docker-compose.local.yml`** (local only, `just up`): `postgres:18.6-alpine`, `valkey/valkey:9.1.2-alpine`, `qdrant/qdrant:v1.19.0`, `rustfs/rustfs:1.0.1`, plus loopback host ports (web 3000, gateway 8080, zitadel 8085, lago-api 3001, rustfs 9000).

Volumes: `gateway_data` (redb + embedding cache), `lago_storage`; locally also `postgres_data`, `qdrant_data`, `rustfs_data`. Telemetry leaves over OTLP/HTTP to ConusAI Cloud (`<platform>/otel/v1/*`); there is no collector or Jaeger in the project.

## 11. Frontend

- **`@conusai/ui`** (Svelte 5, `sideEffects: ["**/*.css"]`) — `tokens.css`, `foundry.css`; components `AppShell, CapabilityCard, PlanBadge, PlanCard, QuotaBanner, ThemeProvider/Script/Switcher, ToastHost, UsageMeter, WorkspaceTree`; features `AgentChatComposer, AgentChatStream, ToolCallCard, WorkspaceExplorer` + `createChatStream.svelte.ts`; plus `capabilities/`, `stores/`, `motion/`, `utils/`.
- **`apps/web`** — SvelteKit (Node adapter); uses `@conusai/{ui,sdk,types}`; backend-for-frontend: server-side Zitadel OIDC (code + PKCE) under `src/routes/auth/*`, tokens in the AES-256-GCM cookie `__Host-epifly_session` (`AUTH_SESSION_KEY`), same-origin proxy of `/v1`, `/api`, `/ui`, `/mcp`, `/admin` to `GATEWAY_URL` with the bearer injected (`src/lib/server/`).
- **`apps/browser-shell`** — SvelteKit (static) + Tauri 2 (`@tauri-apps/{api,plugin-dialog}`); mobile shell `src/lib/mobile/*`; native modules in `src-tauri/src/` (`auth/` — loopback OIDC + PKCE, refresh token in a Stronghold vault; chat_stream, recorder, registration, tabs, telemetry); build-time `VITE_GATEWAY_URL`, `VITE_ZITADEL_ISSUER`, `VITE_ZITADEL_NATIVE_CLIENT_ID`; debug-only WebDriver via feature `e2e` (macOS).

## 12. Architecture Decisions

ADRs in `docs/adr/`: [0007](adr/0007-everything-is-a-capability.md) (Everything is a capability), [0008](adr/0008-orchestration-hook-vs-subexecution.md) (Orchestration hook vs sub-execution).
