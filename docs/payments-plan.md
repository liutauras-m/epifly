# Payments, Billing, OAuth, Quotas & Invoicing — Implementation Plan

> **Reference:** [docs/tasks/payments-task.md](docs/tasks/payments-task.md)
> **Scope:** Replace the current dev HMAC/JWT auth with **Zitadel** (OAuth2/OIDC + tenants + RBAC) and add **Lago + Stripe** for subscriptions, real-time usage metering, quota enforcement, and invoicing.
> **Style:** SRP, Rust 2024 + Axum 0.8, no payment data ever touches our process.

---

## 0. Current-State Audit (what we already have)

### Auth / identity
- **HMAC session** ([`apps/backend/crates/agent-gateway/src/auth/verifier.rs`](apps/backend/crates/agent-gateway/src/auth/verifier.rs)) — `<payload_b64>.<sig_b64>` cookie/header (`conusai_session` / `X-Session-Token`). Issued by SvelteKit ([`apps/web/src/lib/server/session.ts`](apps/web/src/lib/server/session.ts)).
- **HS256 JWT login** ([`apps/backend/crates/agent-gateway/src/routes/auth.rs`](apps/backend/crates/agent-gateway/src/routes/auth.rs)) — `POST /v1/auth/login`, dev-only password check (`DEV_PASSWORD` env). No user store.
- **Tenant extraction** ([`apps/backend/crates/agent-gateway/src/mw/tenant.rs`](apps/backend/crates/agent-gateway/src/mw/tenant.rs)) — priority: Bearer JWT → cookie → `X-Session-Token` → dev fallback. Inserts `ResolvedTenant(TenantContext)` into request extensions.
- **Plan / role types** ([`apps/backend/crates/agent-core/src/context/tenant.rs`](apps/backend/crates/agent-core/src/context/tenant.rs)) — `PlanTier { Free, Pro, Enterprise }`, `UserRole { User, Admin, SuperAdmin }`. Plan‑tier limits hard‑coded in Rust (`max_tokens`, `max_turns`, `rate_limit_rpm`).
- **Plan enforcement** ([`apps/backend/crates/agent-gateway/src/mw/plan.rs`](apps/backend/crates/agent-gateway/src/mw/plan.rs)) — only validates that a plan is present; clamping happens in chat/agent handlers.

### Billing
- **None.** No subscription state, no usage events, no Stripe/Lago, no `billing-core` crate.

### API keys
- **Static env-based** ([`apps/backend/crates/agent-gateway/src/mw/api_key.rs`](apps/backend/crates/agent-gateway/src/mw/api_key.rs)) — `API_KEYS=<blake3_hex>:<tenant_id>:<plan>,...` parsed at boot. No self-serve issuance, rotation, or per-key usage tracking.

### Frontend
- **Login page** ([`apps/web/src/routes/login/+page.svelte`](apps/web/src/routes/login/+page.svelte)) — form posts to local action which signs an HMAC cookie.
- **No** `/account`, `/billing`, `/upgrade`, `/usage` routes.
- **Super-admin Askama UI** under `apps/backend/crates/agent-gateway/templates/`.

### Infrastructure
- [docker-compose.yml](docker-compose.yml): Qdrant + RustFS + agent-gateway. **No** Postgres, Redis, Zitadel, Lago.

### Gaps (delta this plan must close)
1. No OAuth/OIDC; passwordless / social login impossible.
2. No real user / tenant store — `tenant_id` is read from env (`CONUSAI_UI_TENANT_ID`).
3. No billing provider, no Stripe integration, no usage metering, no invoices.
4. Plan tiers are compile-time constants; cannot be changed without redeploy.
5. Quota middleware doesn't actually enforce per-action quotas.
6. No webhook endpoint for billing events; no plan-tier sync to identity claims.
7. No self-serve API-key management, no per-key usage attribution, no referral/discount system, no invoice download UI.

---

## 1. Target Architecture

```
┌──────────┐      OIDC (PKCE)     ┌──────────┐
│ Web/Tauri├─────────────────────▶│ Zitadel  │  Org=Tenant, Project=Roles
└────┬─────┘                      └────┬─────┘
     │  Bearer access_token            │  custom claims: plan_tier, sub_status
     ▼                                 ▼
┌────────────────────────────────────────────┐
│ agent-gateway (Axum)                       │
│  ├─ mw::identity    verify access_token    │
│  ├─ RouterQuotaLayer (extended)            │  ── pre-handler
│  ├─ <handler>                              │
│  └─ mw::meter       BillingProvider.report │  ── post-handler
└──────────┬─────────────────────────────────┘
           │  REST (lago-client)             ▲
           ▼                                 │  webhook (HMAC)
┌──────────────┐  Stripe (PCI)   ┌──────────┴───┐
│ Lago (self)  ├────────────────▶│ Stripe       │
│ plans+events │                  │ Checkout/Sub │
└──────┬───────┘                  └──────────────┘
       │ webhook → /v1/billing/webhooks → update Zitadel claim + SSE
       ▼
   Postgres + Redis (Lago)        TimescaleDB (our usage hot-cache)
```

Single sources of truth:
- **Identity / tenants / roles** → Zitadel (Organizations = tenants).
- **API-key metadata** → hybrid: optional Zitadel `MachineUser` PAT for org-scoped automation, **default** path is our own `ApiKeyService` in `billing-core` (blake3 hashes in `redb`/Postgres) so we get per-key Lago usage attribution, self-serve rotate/revoke UX, and zero hot-path latency. Rationale captured in ADR 012.
- **Plans / subscriptions / usage / invoices / coupons / referrals** → Lago.
- **Payments / cards** → Stripe (Lago wraps it, we never touch).
- **Real-time quota cache** → moka (in-process) backed by TimescaleDB rollups.

---

## 1a. Pricing Strategy (catalog seeded into Lago by `PlanCatalog`)

| Plan | Monthly | Annual (−20%) | Agent turns / day | API calls / day | API limits (RPM / TPM) | Included credits | Storage | Overage / extra unit | Best for |
|---|---|---|---|---|---|---|---|---|---|
| **Free** | $0 | — | 50 | 100 | 10 / 1 k | 0 | 1 GB | hard cap (HTTP 429) | trial / hobby / POC |
| **Pro** | $20 | $17/mo | 500 | 5 000 | 60 / 20 k | 2 000/mo | 25 GB | $0.01 turn / $0.000003 token | power users + small apps |
| **Team** | $80 base + $15/seat | $64 + $12/seat | 2 000 shared | 20 000 shared | 300 / 100 k | 10 000/mo | 100 GB | $0.008 turn / $0.0000025 token | small teams |
| **Enterprise** | custom (≥ $500) | custom | unlimited | unlimited | 1 000+ / custom | custom | unlimited | negotiated | production scale |

Rolling 24 h windows for free-tier limits (feels generous, blocks abuse).

**Discounts & growth (all native Lago coupon primitives — zero custom code):**
- **Annual:** 20 % off (`coupon: ANNUAL20`, applies on plan switch).
- **Promo codes:** one-time 25 %.
- **Referral programme** — each user gets a shareable link `https://app.conusai.com/signup?ref=<ulid>`:
  - Referred user → 30 % off first 3 months.
  - Referrer → $20 Lago credit (or 1 free month) when referred user upgrades to a paid plan.
  - Referral code is stored as a Zitadel custom user claim `urn:conusai:ref_by`; redemption recorded in Lago via wallet credit.

---

## 2. Phase Breakdown

The work is split into 8 phases (plus a small in-place evolution — 5b). Each phase is independently shippable, behind a feature flag where it touches request paths.

### Phase 0 — ADR + scaffolding *(foundational, no behavior change)*
- Create [`docs/adr/012-zitadel-lago-auth-billing.md`](docs/adr/012-zitadel-lago-auth-billing.md) with the architecture above and "Status: Proposed". Document the **hybrid API-key decision** (when to use Zitadel PAT vs custom `ApiKeyService`).
- Add workspace deps in `apps/backend/Cargo.toml`:
  - `zitadel = { version = "5.7", features = ["axum", "credentials", "interceptors"] }`
  - `lago-client = "0.1.23"`
  - `oauth2 = "5"` (used only for the OIDC PKCE flow on the SvelteKit side via fetch; Rust side uses zitadel crate).
  - `moka = { version = "0.12", features = ["future"] }`
  - `figment` (already in workspace) used for `PlanCatalog` TOML overrides.
- Create new crate `crates/billing-core` (lib) and add to workspace members.
- Add `sqlx` migrations under `apps/backend/migrations/` for `api_keys` and `usage_rollups` tables (tenant-isolated via Postgres RLS).
- Extend `crates/agent-core` deps to optionally include `zitadel`.
- Feature flag: `CONUSAI_IDENTITY_PROVIDER=legacy|zitadel` (default `legacy` until Phase 3 ships). *(Renamed from `CONUSAI_AUTH_PROVIDER` for clarity.)*

**Acceptance:** `cargo check -p billing-core` passes; new crates compile empty; legacy auth unaffected.

---

### Phase 1 — `agent-core` identity abstractions *(refactor only)*

Goal: stop hard-coding the auth path so Zitadel can slot in without rewrites.

1. New module [`agent-core/src/identity/mod.rs`](apps/backend/crates/agent-core/src/identity/mod.rs):
   ```rust
   /// Canonical authenticated principal — carried through every request.
   /// (Renamed from `IdentityContext` to avoid "Context" overload; pairs with `TenantContext`.)
   #[derive(Clone, Debug)]
   pub struct AuthIdentity {
       pub user_id: String,
       pub tenant_id: TenantId,
       pub email: Option<String>,
       pub roles: Vec<UserRole>,
       pub plan_tier: PlanTier,
       pub subscription_status: SubscriptionStatus, // Active|PastDue|Canceled|Trialing
       pub raw_claims: serde_json::Value,
   }

   #[async_trait]
   pub trait IdentityProvider: Send + Sync + 'static {
       async fn verify_access_token(&self, token: &str) -> Result<AuthIdentity, AuthError>;
       async fn user_info(&self, sub: &str) -> Result<AuthIdentity, AuthError>;
       async fn health(&self) -> Result<(), AuthError>;
   }

   /// Supertrait — every TenantService is an IdentityProvider.
   /// (Renamed from `TenantManager`: "Service" matches `CapabilityService` pattern.)
   #[async_trait]
   pub trait TenantService: IdentityProvider {
       async fn create_tenant(&self, name: &str, owner_email: &str) -> Result<TenantCreated, AuthError>;
       async fn list_tenants(&self) -> Result<Vec<TenantSummary>, AuthError>;
       async fn invite_user(&self, tenant_id: &TenantId, email: &str, role: UserRole) -> Result<(), AuthError>;
       async fn update_plan_claim(&self, tenant_id: &TenantId, tier: PlanTier, status: SubscriptionStatus) -> Result<(), AuthError>;
   }
   ```
2. `TenantContext::from_identity(AuthIdentity, workspace_root) -> Self` for back-compat.
3. Re-export from `agent_core::identity::*`.
4. Add `SubscriptionStatus` to `context/tenant.rs` and serialize on `TenantClaims`.

**Acceptance:** existing tests green; no runtime change yet.

---

### Phase 2 — `ZitadelProvider` impl + Docker Zitadel

1. [docker-compose.yml](docker-compose.yml) additions under the **existing** `full` and `infra` profiles (matches current compose philosophy — no new profile):
   - `postgres` (shared, also used later by Lago) — `postgres:17-alpine`.
   - `zitadel` (`ghcr.io/zitadel/zitadel:v3.0.0-stable`) with `zitadel-init` one-shot service to create master key + project + roles via `defaults.yaml`.
   - Volume `zitadel_data`, healthcheck on `:8080/debug/healthz`.
2. Config dir [`docker/zitadel/`](docker/zitadel/):
   - `defaults.yaml` — declares Project `conusai-agent-gateway`, roles `user`, `admin`, `super_admin`, custom claim mapper for `plan_tier` + `subscription_status` (Zitadel "Actions v2" trigger `pre_userinfo` + `pre_access_token`).
   - `bootstrap.sh` — uses `zitadel-tools` to seed an admin and a test org.
3. New file [`agent-core/src/identity/zitadel.rs`](apps/backend/crates/agent-core/src/identity/zitadel.rs):
   - `ZitadelConfig::from_env()` → `ZITADEL_DOMAIN`, `ZITADEL_AUDIENCE`, `ZITADEL_INTROSPECTION_KEY` (JWT profile JSON), `ZITADEL_MGMT_KEY`.
   - `ZitadelProvider` wraps:
     - `zitadel::axum::introspection::IntrospectionStateBuilder` for token verification.
     - `zitadel::api::clients::ClientBuilder` (gRPC `ManagementServiceClient`) for tenant ops.
   - Implements `IdentityProvider` and `TenantManager`.
   - Maps Zitadel `urn:zitadel:iam:org:project:roles` → `Vec<UserRole>`; reads `urn:conusai:plan_tier` claim.
4. Unit tests with `mockito` for the introspection HTTP path; integration test using `testcontainers` zitadel image (gated `--features integration`).

**Acceptance:** `cargo test -p agent-core --features integration zitadel::` passes; running `docker compose --profile infra up` brings Zitadel up healthy.

---

### Phase 3 — Wire Zitadel into `agent-gateway` (behind flag)

1. [`apps/backend/crates/agent-gateway/src/state.rs`](apps/backend/crates/agent-gateway/src/state.rs): add `pub identity: Arc<dyn TenantService>` to `AppState`. In `from_env`, branch on `CONUSAI_IDENTITY_PROVIDER`:
   - `legacy` → `LegacyIdentityProvider` (wraps existing `auth::extract_from_headers` + `mw::tenant` JWT path) so the trait works for both code paths.
   - `zitadel` → `ZitadelProvider::from_env()`.
2. New middleware [`mw/identity.rs`](apps/backend/crates/agent-gateway/src/mw/identity.rs) (canonical name, matches `IdentityProvider`):
   - Extracts `Authorization: Bearer`, calls `identity.verify_access_token`, inserts `ResolvedTenant(TenantContext::from_identity(..))` into extensions.
   - Falls back to legacy cookie path if `provider == legacy` (preserves Phase 0–2 behavior).
3. Wire in `main.rs` *before* `mw::tenant::extract_tenant`; later phases will fully replace `mw::tenant` with `mw::oidc`.
4. SvelteKit changes ([`apps/web/src/lib/server/session.ts`](apps/web/src/lib/server/session.ts)):
   - New `ZitadelOidcAdapter` implementing `SessionAdapter` — uses `oauth4webapi` for PKCE, stores `id_token` + `access_token` in cookie (encrypted with `UI_SESSION_KEY`).
   - Activated by `AUTH_PROVIDER=zitadel` env var.
   - New routes `/auth/login` (redirect to Zitadel), `/auth/callback`, `/auth/logout` (RP-initiated logout).
5. Tauri `apps/browser-shell` ([`src/lib/sdk.ts`](apps/browser-shell/src/lib/sdk.ts)): add OS-browser PKCE flow via `tauri-plugin-oauth`; store `access_token` in keychain through existing `set_device_token` Rust command.

**Acceptance:** With `CONUSAI_IDENTITY_PROVIDER=zitadel`, full login → call `/v1/agent/...` works on web + Tauri. With `legacy`, nothing changes. e2e test in [e2e/web/auth.spec.ts](e2e/web/auth.spec.ts) updated.

---

### Phase 4 — `billing-core` crate (provider trait + Lago impl + quota cache)

Layout (canonical, mirrors `agent-core`):
```
crates/billing-core/
├── Cargo.toml
└── src/
    ├── lib.rs        ← prelude + re-exports (BillingProvider, QuotaEnforcer, BillableAction, ...)
    ├── provider.rs   ← BillingProvider trait + BillingError
    ├── service.rs    ← LagoProvider impl + ApiKeyService + ReferralService
    ├── catalog.rs    ← PlanCatalog + PlanDefinition (figment-loadable; CONUSAI_PLAN_CATALOG_PATH override)
    ├── metering.rs   ← BillableAction, UsageEvent, batched fire-and-forget queue
    ├── quota.rs      ← QuotaEnforcer (moka cache + Timescale rollups)
    ├── types.rs      ← Subscription, Invoice, CheckoutSession, SubscriptionStatus, QuotaDecision
    └── store/
        └── mod.rs    ← redb / Postgres metadata (api_keys, usage_event backups, idempotency keys)
```

`QuotaEnforcer` lives **inside** `billing-core` (SRP); `AppState` holds only `Arc<QuotaEnforcer>` + `Arc<dyn BillingProvider>`.

Key contracts:
```rust
#[async_trait]
pub trait BillingProvider: Send + Sync + 'static {
    async fn create_or_update_subscription(
        &self, tenant_id: &TenantId, plan_key: &str, return_url: &str,
    ) -> Result<CheckoutSession, BillingError>;
    async fn cancel_subscription(&self, tenant_id: &TenantId) -> Result<(), BillingError>;
    async fn get_subscription(&self, tenant_id: &TenantId) -> Result<Subscription, BillingError>;
    async fn report_usage(&self, event: UsageEvent) -> Result<(), BillingError>; // fire-and-forget; spawns to batched queue
    async fn list_invoices(&self, tenant_id: &TenantId) -> Result<Vec<Invoice>, BillingError>;
    async fn portal_url(&self, tenant_id: &TenantId, return_url: &str) -> Result<Url, BillingError>;
}

pub struct QuotaDecision { pub allowed: bool, pub remaining: Option<u64>, pub reset_at: Option<DateTime<Utc>>, pub reason: Option<String> }
pub struct QuotaEnforcer { /* moka<(TenantId, BillableAction), Counter> + Postgres pool */ }
impl QuotaEnforcer {
    #[tracing::instrument(skip(self))]
    pub async fn check(&self, ctx: &AuthIdentity, action: BillableAction, qty: u64) -> QuotaDecision;
    pub async fn record(&self, ctx: &AuthIdentity, action: BillableAction, qty: u64);
}

// BillableAction is the canonical metering vocabulary — each variant maps 1:1 to a Lago billable_metric.
pub enum BillableAction { AgentTurn, ApiCall, CapabilityInvoke, Token, StorageGB, FileUpload }
```

All public methods get `#[tracing::instrument]` (OTel spans already wired in our stack).

LagoProvider responsibilities:
- Idempotent `ensure_customer(tenant_id, email)` on first call.
- Maps our `PlanTier` ↔ Lago plan codes (`free`, `pro`, `team`, `enterprise`) defined in `catalog.rs`.
- `report_usage` is **fire-and-forget**: enqueues to a tokio MPSC (1 s flush interval) so the request handler returns in <1 ms. Backpressure spills to disk via `store/`. Idempotency via `transaction_id = uuid_v7()`.
- Webhook-signature verification helper (HMAC-SHA256, header `X-Lago-Signature`).

`ApiKeyService` (in `service.rs`) — **default path** for API keys (hybrid model):
- Issues `ck_live_<22b62>` plaintext, persists `blake3(plaintext)` + metadata (`name`, `scopes`, `tenant_id`, `created_at`, `last_used_at`) in `store/`.
- Optional Zitadel PAT linkage stored as `zitadel_pat_id` reference (used only for org-scoped Machine Users).
- `resolve(blake3_hex) -> Option<ApiKeyEntry>` is hot-path; backed by moka.

`PlanCatalog` (in `catalog.rs`) seeded at boot via `LagoProvider::ensure_plans()` so plans live as code (idempotent upsert). Loaded via `figment` from compiled defaults + optional `CONUSAI_PLAN_CATALOG_PATH=/etc/conusai/plans.toml`:
| Code | Monthly | `agent_turn` (overage) | `token` (overage) | `storage_gb` | Quotas |
|---|---|---|---|---|---|
| free | $0 | n/a (hard cap 50/day) | n/a | 1 | 50 turns/day, 10 invokes/day, no opus |
| pro | $20 | $0.01 over 500/day | $0.000003 | 25 | 500 turns/day |
| team | $80/seat | $0.008 over 2k/day | $0.0000025 | 100 | shared workspace |
| enterprise | custom | negotiated | negotiated | unlimited | dedicated throughput |

**Acceptance:** `cargo test -p billing-core` green; integration test against `lago-api` testcontainer creates customer → reports event → fetches subscription.

---

### Phase 5 — Quota + metering middleware in `agent-gateway`

1. `AppState` gets `pub billing: Arc<dyn BillingProvider>` and `pub quota: Arc<QuotaEnforcer>`.
2. **Extend the existing `RouterQuotaLayer`** ([`apps/backend/crates/agent-gateway/src/mw/`](apps/backend/crates/agent-gateway/src/mw/)) — do NOT create a new `mw/quota.rs`. Add `Arc<dyn BillingProvider>` + `Arc<QuotaEnforcer>` to its config; it already owns the request-path → action mapping pattern (`RouterQuotaConfig`). `QuotaEnforcer::check` injects the `QuotaDecision` into request extensions so handlers can render rich 429 JSON. On `allowed=false` → `429` + `Retry-After` header + JSON `{ code, plan_tier, upgrade_url }`.
3. Replace [`mw/plan.rs`](apps/backend/crates/agent-gateway/src/mw/plan.rs) — keep the presence check, delete hard-coded clamp comments (clamping moves into `RouterQuotaLayer` using `AuthIdentity.plan_tier.max_*`).
4. New [`mw/meter.rs`](apps/backend/crates/agent-gateway/src/mw/meter.rs) — implemented as a proper `tower::Layer` wrapping the response future (canonical Axum 0.8 pattern, cleaner than `next.run()`). Reads response status + `AgentTurnStats { tokens, model, duration_ms }` from response extensions, then `billing.report_usage(...)` (fire-and-forget) + `quota.record(...)`.
5. In [`agent-core/src/agent/builder.rs`](apps/backend/crates/agent-core/src/agent/builder.rs) (and chat stream handler), insert `AgentTurnStats` into response extensions so `mw::meter` can read it.
6. Layer order in [`main.rs`](apps/backend/crates/agent-gateway/src/main.rs) `protected_router`:
   ```
   .layer(mw::meter::MeterLayer::new(...)) // outermost (post; tower::Layer)
   .layer(RouterQuotaLayer::new(...))      // extended: BillingProvider + QuotaEnforcer
   .layer(mw::plan::enforce_plan)
   .layer(mw::identity::extract_identity)  // canonical name; replaces extract_tenant when zitadel enabled
   .layer(mw::api_key::extract_api_key)
   ```

**Acceptance:** integration test: 51st free-tier `POST /v1/agent/run` in 24h → `429`; usage event visible in Lago.

#### Phase 5b — Evolve `mw/api_key.rs` (no new file)

Replace the env-var `API_KEYS` lookup in [`mw/api_key.rs`](apps/backend/crates/agent-gateway/src/mw/api_key.rs) with `ApiKeyService::resolve(blake3_hash) -> Option<ApiKeyEntry { key_id, tenant_id, plan, scopes }>`. Keep the existing function name `extract_api_key` and middleware position so call-sites in `main.rs` are unchanged. The resolver:
- Reads from a moka cache (60 s TTL) backed by `RedbMetadataStore::api_keys` (or Postgres when present).
- Inserts `key_id` into request extensions so `mw::meter` can stamp `UsageEvent.metadata.api_key_id` for per-key invoicing & dashboards.
- Increments `conusai_api_key_calls_total{key_id, plan}` Prometheus counter.

Legacy `API_KEYS` env var still honoured (parsed once, written into the same store on first boot) for zero-downtime cutover.

---

### Phase 6 — Subscription routes + Stripe webhook + API keys + referrals

1. New router file [`routes/billing.rs`](apps/backend/crates/agent-gateway/src/routes/billing.rs) — mounted under protected `/v1/billing/*`:
   - `GET  /v1/billing/plans` → `PlanCatalog::list()` (incl. annual prices + applicable coupons).
   - `GET  /v1/billing/subscription` → current `Subscription`.
   - `POST /v1/billing/subscriptions` `{ plan_key, billing_cycle, coupon_code?, return_url }` → `CheckoutSession { url, expires_at }`.
   - `POST /v1/billing/portal` `{ return_url }` → Lago/Stripe customer portal URL.
   - `DELETE /v1/billing/subscription` → cancel at period end.
   - `GET  /v1/billing/invoices` → list (paginated, signed PDF download URL via Lago).
   - `GET  /v1/billing/invoices/:id/pdf` → 302 to Lago signed URL.
   - `GET  /v1/billing/usage?from&to&group_by=action|api_key` → aggregated usage (Timescale rollup).
   - **API keys:** `GET /v1/api-keys`, `POST /v1/api-keys` `{ name, scopes }` (returns plaintext **once**), `DELETE /v1/api-keys/:id`, `POST /v1/api-keys/:id/rotate`.
   - **Referrals:** `GET /v1/referrals/me` (link + stats), `POST /v1/referrals/redeem` `{ code }` (idempotent; only valid pre-first-payment).
2. New **public** route `POST /v1/billing/webhooks` ([`routes/billing_webhook.rs`](apps/backend/crates/agent-gateway/src/routes/billing_webhook.rs)) — mounted in `public_router` behind a thin `LagoWebhookVerifier` middleware (signature check only, ~15 LOC):
   - HMAC verify against `LAGO_WEBHOOK_SECRET`.
   - Match `event_type`:
     - `subscription.started|updated|terminated` → `identity.update_plan_claim(...)` so the next access-token issued by Zitadel carries the new tier; also push `RealtimeService` event so live UIs flip plan badges.
     - `invoice.payment_succeeded` / `invoice.payment_failed` → audit log + email (via existing `email-core` if present, else log).
     - `customer.usage.threshold_reached` → SSE warning to user.
   - Idempotency: store `event_id` in `redb` (or Postgres) with 90-day TTL; reject replays.
3. Admin-only routes `/admin/billing/*`:
   - `POST /admin/billing/credits` — add credits.
   - `POST /admin/billing/cancel/:tenant_id` — manual cancel.
   - `GET /admin/billing/dashboard` — proxied Lago analytics summary.
4. Bootstrap on first start ([`bootstrap.rs`](apps/backend/crates/agent-gateway/src/bootstrap.rs)):
   - `LagoProvider::ensure_plans()` (seed catalog incl. monthly + annual).
   - `LagoProvider::ensure_coupons()` (`ANNUAL20`, `REFERRAL30`, promo template).
   - For every Zitadel Organization without a Lago customer → create customer + Free subscription.
   - Migrate any `API_KEYS` env entries into `ApiKeyService` store (idempotent).

**Acceptance:** Webhook signature replay test rejected; manual upgrade flow E2E (Phase 7) lights up plan badge within 2s of Stripe redirect.

---

### Phase 7 — Frontend: upgrade flow, usage UI, account page

1. SvelteKit (`apps/web`):
   - [`src/routes/account/+page.svelte`](apps/web/src/routes/account/+page.svelte) — profile, plan badge, Manage Billing button (calls `POST /v1/billing/portal`).
   - [`src/routes/account/billing/+page.server.ts`](apps/web/src/routes/account/billing/+page.server.ts) — loads plans + current subscription + applicable coupons.
   - [`src/routes/account/billing/+page.svelte`](apps/web/src/routes/account/billing/+page.svelte) — plan cards, monthly/annual toggle (annual highlights `Save 20 %`), coupon-code field, "Upgrade" POSTs to `/v1/billing/subscriptions` then `window.location = checkout_url`.
   - [`src/routes/account/usage/+page.svelte`](apps/web/src/routes/account/usage/+page.svelte) — `GET /v1/billing/usage` + `@conusai/ui` charts; toggle between per-action and per-API-key views.
   - [`src/routes/account/invoices/+page.svelte`](apps/web/src/routes/account/invoices/+page.svelte) — paginated table, Download-PDF button hits `/v1/billing/invoices/:id/pdf`.
   - [`src/routes/account/api-keys/+page.svelte`](apps/web/src/routes/account/api-keys/+page.svelte) — list / create / rotate / revoke; plaintext key shown **once** in a copy-to-clipboard modal with security warning.
   - [`src/routes/account/referrals/+page.svelte`](apps/web/src/routes/account/referrals/+page.svelte) — personal link, stats (clicks, signups, paid conversions, credits earned), social share buttons.
   - [`src/routes/signup/+page.server.ts`](apps/web/src/routes/signup/+page.server.ts) — reads `?ref=<ulid>` query param, persists to a short-lived cookie, replays into Zitadel signup as `urn:conusai:ref_by` claim.
2. Shared UI in [`packages/ui`](packages/ui):
   - `<PlanBadge tier=... status=... />`
   - `<PlanCard ... billingCycle=... onUpgrade={...} />`
   - `<UsageMeter action=... used=... limit=... />`
   - `<QuotaBanner />` — listens on `RealtimeService` SSE for `quota.exceeded` and `subscription.updated`.
   - `<ApiKeyTable />` + `<ApiKeyRevealModal />` (one-time plaintext display).
   - `<ReferralCard />` (link, copy button, stats).
3. Tauri shell (`apps/browser-shell`) — same routes (already shares SvelteKit code), ensures Stripe Checkout opens in **system browser** via `tauri-plugin-shell::open` (Apple Pay / 3DS will not work inside WKWebView).
4. Askama super-admin templates: add `templates/admin/billing.html` rendering tenant list + plan/usage + manual override actions; `templates/admin/referrals.html` for fraud monitoring (chargeback rate, self-referral detection).

**Acceptance:** Playwright E2E `e2e/web/billing.spec.ts` — log in (Zitadel test user) → upgrade to Pro → Stripe test card → returns to `/account/billing` with `Pro` badge within 5s.

---

### Phase 8 — Migration, docs, cleanup

1. Migration script [`scripts/migrate-to-zitadel-lago.ts`](scripts/migrate-to-zitadel-lago.ts):
   - Read existing tenants from `redb` (`MetadataStore::list_tenants`).
   - For each: create Zitadel Organization (idempotent), create Lago customer + Free subscription, write `tenant_id` mapping back to redb.
2. Flip default `CONUSAI_IDENTITY_PROVIDER=zitadel`; delete legacy `routes/auth.rs::login` (move to deprecated `/v1/auth/legacy/login` for 30 days).
3. Remove hard-coded plan limits from `agent-core/src/context/tenant.rs` — `max_tokens()` etc. now read from `PlanCatalog` (cached). Keep `Display` and serde.
4. Update [docs/arch.md](docs/arch.md), [docs/auth-plan.md](docs/auth-plan.md); promote ADR 012 to "Accepted".
5. Telemetry: OTel spans `billing.report_usage`, `quota.check`, `oidc.verify`; Prometheus counters `conusai_quota_denied_total{action,plan}`, `conusai_billing_webhook_total{event,result}`, histogram `conusai_oidc_verify_duration_seconds`.
6. Runbook [`docs/ops/billing.md`](docs/ops/billing.md): rotating Stripe keys, replaying Lago webhooks, refunding, tenant lookup.
7. Nightly reconciliation job: diff Zitadel `plan_tier` claims vs Lago active subscriptions; alert on drift.

**Acceptance:** legacy auth path removed from default code path; all new tenants flow through Zitadel; nightly job reconciles Lago vs Zitadel claims (alerts on drift).

---

## 3. Configuration Surface (env vars)

```
# Auth
CONUSAI_IDENTITY_PROVIDER=zitadel|legacy       # Phase 0 flag (renamed from CONUSAI_AUTH_PROVIDER)
ZITADEL_DOMAIN=https://auth.conusai.com
ZITADEL_AUDIENCE=conusai-agent-gateway
ZITADEL_INTROSPECTION_KEY=/etc/secrets/zitadel-introspection.json
ZITADEL_MGMT_KEY=/etc/secrets/zitadel-mgmt.json
SUPER_ADMIN_EMAILS=...                          # already exists

# Billing
LAGO_API_URL=http://lago-api:3000
LAGO_API_KEY=...
LAGO_WEBHOOK_SECRET=...
STRIPE_SECRET_KEY=sk_live_...                   # passed to Lago, not used directly
STRIPE_PUBLIC_KEY=pk_live_...                   # SvelteKit
BILLING_RETURN_URL=https://app.conusai.com/account/billing
CONUSAI_PLAN_CATALOG_PATH=/etc/conusai/plans.toml   # optional — overrides compiled catalog
CONUSAI_REFERRAL_CREDIT_USD=20                       # referrer reward
CONUSAI_REFERRAL_DISCOUNT_PCT=30                     # referred discount, first 3 months
API_KEYS=                                            # legacy bootstrap only — migrated to ApiKeyService on first boot

# Infra
POSTGRES_URL=postgres://conusai:...@postgres:5432/conusai
REDIS_URL=redis://redis:6379/0
TIMESCALE_URL=postgres://...@timescale:5432/usage
```

---

## 4. Test Strategy

| Layer | Tooling | What |
|---|---|---|
| Unit | `cargo test` | Provider mocks, quota math, webhook signature verification, plan catalog upsert idempotency. |
| Integration | `testcontainers` (zitadel, lago-api, postgres, redis) | Full token verify → quota → meter → webhook round-trip. |
| Contract | `wiremock` | Stripe-side simulation for Lago test mode. |
| E2E | Playwright ([e2e/web](e2e/web)) | Login (Zitadel), upgrade (Stripe test card), quota 429, portal redirect. |
| Load | `k6` | Sustain 1k RPS through `mw::quota` + `mw::meter` with <5ms p95 added latency (moka cache hit). |
| Security | Manual + `cargo audit` | Webhook replay, signature stripping, JWT `aud`/`iss` mismatch, PKCE downgrade, IDOR on `/v1/billing/*`. |

---

## 5. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Stripe webhook lag → user sees Free after paying | Lago webhook fires within 1s; UI also polls `GET /v1/billing/subscription` for 10s post-redirect; SSE pushes update. |
| Zitadel down → all auth fails | `IntrospectionLayer` caches valid tokens (TTL = `exp`); Tauri caches last `IdentityContext` for offline 5-minute grace. |
| Usage event loss under load | Tokio batched queue with disk overflow (`sled`/`redb`); replay on startup; idempotent `transaction_id`. |
| Quota cache drift across replicas | moka per-process + 30s Timescale re-sync; counters are advisory, Lago is the billing source-of-truth. |
| Plan-tier claim staleness in JWT | `update_plan_claim` invalidates Zitadel session; access tokens are short-lived (5 min); access-token refresh picks up new claim. |
| PCI scope creep | Stripe Checkout only; never proxy card fields; CSP `frame-src https://checkout.stripe.com`. |
| Hybrid API-key confusion (custom vs Zitadel PAT) | ADR 012 documents the decision tree; default = `ApiKeyService`; PAT only for org-scoped Machine Users. |

---

## 6. Effort & Sequencing

*(Refined twice: reusing `RouterQuotaLayer` + canonical naming + hybrid API-key model + restructured `billing-core` saves ~6 AI-hours vs original.)*

| Phase | Depends on | AI-hours |
|---|---|---|
| 0 — ADR + scaffolding (incl. sqlx migrations) | — | 1 |
| 1 — identity abstractions (`AuthIdentity` / `TenantService`) | 0 | 2 |
| 2 — ZitadelProvider + docker | 1 | 3 |
| 3 — gateway wiring + SvelteKit OIDC | 2 | 3 |
| 4 — billing-core (`BillingProvider` + `QuotaEnforcer` + `ApiKeyService` + `ReferralService`) | 1 | 4 |
| 5 — quota (extend layer) + meter (`tower::Layer`) | 4 | 2 |
| 5b — evolve `mw/api_key.rs` to use `ApiKeyService` | 4 | 1 |
| 6 — billing/api-key/referral routes + webhook | 4, 5 | 3 |
| 7 — frontend (billing/usage/invoices/api-keys/referrals) | 6 | 5 |
| 8 — migration + cleanup + reconciliation job | 7 | 2 |
| **Total** | | **~26** |

Phases 0–3 can ship as one PR (auth foundation), 4–6 as the second (billing core), 7–8 as the third (UI + cutover). Each PR is independently revertable via the `CONUSAI_IDENTITY_PROVIDER` flag.

---

## 7. Definition of Done

- [ ] User signs up via Zitadel-hosted UI (email/password + Google + GitHub); `?ref=` query param flows through to Zitadel claim.
- [ ] New tenant auto-created in Zitadel + Lago (Free plan).
- [ ] Free user hits quota → `429` + upgrade prompt with deep link to `/account/billing`.
- [ ] User clicks Upgrade → chooses monthly/annual → Stripe Checkout → returns → `Pro` badge + new limits live within 5 s.
- [ ] Annual upgrade applies `ANNUAL20`; coupon field accepts promo codes.
- [ ] Referral flow: signup with `?ref=` → referred user gets 30 % off first 3 months; on referred user's first paid invoice, referrer wallet credited $20.
- [ ] Usage dashboard shows turns / tokens / API calls / storage in near real-time, groupable by action **or** API key.
- [ ] Invoices page lists past invoices with PDF download.
- [ ] User can self-serve issue / rotate / revoke API keys; per-key usage visible.
- [ ] 3rd-party app using `X-API-Key` is rate-limited per the issuing tenant's plan; usage attributed to that key in Lago.
- [ ] Failed payment → Lago dunning → final failure downgrades plan claim and notifies user via SSE.
- [ ] Super-admin can issue credits, cancel subscriptions, view per-tenant revenue + referral fraud dashboard.
- [ ] All payment data stays in Stripe; our DB stores only `tenant_id ↔ lago_customer_id` mapping, API-key hashes, and aggregated usage rollups.
- [ ] OWASP Top-10 review passed; webhook replays rejected; PKCE enforced; CSP locked down; API-key plaintext shown exactly once.
- [ ] Legacy HMAC/JWT path remains behind `CONUSAI_IDENTITY_PROVIDER=legacy` for 30 days, then deleted.
