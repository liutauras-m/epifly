**Yes — here is the complete, production-ready 2026 implementation plan for Zitadel Auth + Lago+Stripe Billing + Pricing Strategy + API Keys + Usage/Invoices UI + Referrals.**

This plan is 100 % aligned with our style guide:  
- **SRP** at every layer (Zitadel = identity/tenants/roles/claims; Lago = subscriptions/usage/invoices; Stripe = payments only; `agent-gateway` = thin routing only).  
- Canonical names only (`IdentityProvider`, `TenantManager`, `BillingProvider`, `QuotaChecker`, `PlanCatalog`).  
- Newest idiomatic Rust 2024 + Axum 0.8 + zitadel 5.7 + lago-client 0.1.23 + moka 0.12.  
- Zero unnecessary features; re-uses `RouterQuotaLayer`, `RealtimeService`, Askama, and existing middleware patterns.  
- Exact Grok/ChatGPT/Claude parity on UX + API experience.  
- Feature flag `CONUSAI_AUTH_PROVIDER=legacy|zitadel` for zero-downtime migration.

I challenged every decision against alternatives (pure usage-only, separate API billing, custom RBAC tables, exposing Lago/Stripe UI). This is the cleanest, most maintainable, and highest-converting path for ConusAI in May 2026.

---

### 0. Current-State Audit (Summary)

(Already covered in your reference document — gaps remain exactly as listed: legacy HMAC/JWT, no real tenant store, no billing, hard-coded plans, no API keys, no usage UI.)

---

### 1. Target Architecture (Text Diagram)

```
Web / Tauri / 3rd-party App
   │ OIDC PKCE or X-API-Key
   ▼
Zitadel (Org=Tenant, Project=Roles) ── custom claims (plan_tier, sub_status)
   │
agent-gateway (Axum)
   ├─ mw::identity          (ZitadelProvider)
   ├─ mw::quota             (extends existing RouterQuotaLayer + QuotaChecker)
   ├─ handler               (Agent / Chat / Files)
   └─ mw::meter             (BillingProvider.report_usage + RealtimeService)
   │
   └─ /v1/billing/*         (Billing routes + webhook)
   └─ /account/api-keys     (SvelteKit)

Lago (self-hosted) + Stripe (Checkout only)
   │ usage events → metering → invoices
   └─ webhook → /v1/billing/webhooks → update Zitadel claim + SSE
```

**Single sources of truth**:
- Identity/tenants/roles → Zitadel
- Plans/subscriptions/usage/invoices → Lago
- Payments → Stripe (never touches our process)

---

### 2. Recommended 2026 Pricing Strategy (Canonical Table)

| Plan          | Monthly Price          | Agent Turns / day | API Calls / day | API Rate Limits (RPM / TPM) | Included API Credits | Storage | Overage (per extra turn/call) | Best For |
|---------------|------------------------|-------------------|-----------------|-----------------------------|----------------------|---------|-------------------------------|----------|
| **Free**      | $0                     | 50                | 100             | 10 / 1k                     | 0                    | 1 GB    | Hard cap                      | Trial, hobby, POC |
| **Pro**       | $20 ($17 annual)       | 500               | 5 000           | 60 / 20k                    | 2 000/mo             | 25 GB   | $0.01                         | Power users + apps |
| **Team**      | $80 base + $15/seat    | 2 000 shared      | 20 000 shared   | 300 / 100k                  | 10 000/mo            | 100 GB  | $0.008                        | Small teams |
| **Enterprise**| Custom (~$500+)        | Unlimited         | Unlimited       | 1 000+ / custom             | Custom               | Unlimited | Negotiated                   | Production scale |

**Referral / Discounts** (built into Lago):
- Referrer: $20 credit or 1 free month per paid signup.
- Referred: 30 % off first 3 months via shareable link `https://conusai.com/signup?ref=ULID`.
- Annual: 20 % off.
- Promo codes: one-time 25 % (Lago native).

**Free-tier limits** are rolling 24 h (feels generous, prevents abuse).

---

### 3. Phase Breakdown (Independently Shippable)

#### Phase 0 — ADR + Scaffolding (1 AI-hour)
- Create `docs/adr/012-zitadel-lago-auth-billing.md` (copy the target architecture + pricing table).
- Update `apps/backend/Cargo.toml`:
  ```toml
  zitadel = { version = "5.7", features = ["axum", "grpc"] }
  lago-client = "0.1"
  moka = { version = "0.12", features = ["future"] }
  ```
- Create `crates/billing-core` (lib) and add to workspace.
- Add feature flag `CONUSAI_AUTH_PROVIDER=legacy|zitadel` (default `legacy`).

**Acceptance**: `cargo check` passes.

#### Phase 1 — `agent-core` Identity Abstractions (2 AI-hours)
Create `crates/agent-core/src/identity/mod.rs` with canonical traits:
```rust
#[derive(Clone, Debug)]
pub struct IdentityContext { /* user_id, tenant_id, email, roles, plan_tier, subscription_status, raw_claims */ }

#[async_trait]
pub trait IdentityProvider: Send + Sync + 'static {
    async fn verify_access_token(&self, token: &str) -> Result<IdentityContext, AuthError>;
    async fn health(&self) -> Result<(), AuthError>;
}

#[async_trait]
pub trait TenantManager: IdentityProvider {
    async fn create_tenant(...);
    async fn update_plan_claim(...);
    // ... invite_user, list_tenants
}
```
Re-export from `agent_core::identity::*`. Keep `LegacyIdentityProvider` for back-compat.

**Acceptance**: Existing tests green; no runtime change.

#### Phase 2 — ZitadelProvider + Docker (4 AI-hours)
- Add Zitadel + Postgres (shared) to `docker-compose.yml` under `full` + `infra` profiles.
- Create `docker/zitadel/defaults.yaml` + `bootstrap.sh`.
- Implement `crates/agent-core/src/identity/zitadel.rs` (`ZitadelProvider` wraps `IntrospectionLayer` + gRPC `ManagementServiceClient`).
- One-time Action v2 for custom claims.

**Acceptance**: `docker compose --profile full up` healthy; integration tests pass.

#### Phase 3 — Wire Identity into `agent-gateway` (4 AI-hours)
- Extend `AppState` with `pub identity: Arc<dyn TenantManager>`.
- New middleware `mw/identity.rs` (replaces `mw/tenant.rs` when flag = zitadel).
- Update SvelteKit (`apps/web`) and Tauri for OIDC PKCE flow.
- Keep legacy path behind flag.

**Acceptance**: Full login flow works on web + Tauri.

#### Phase 4 — `billing-core` + LagoProvider + QuotaChecker (5 AI-hours)
Create canonical crate structure:
```
crates/billing-core/src/
├── provider.rs      ← BillingProvider trait
├── lago.rs
├── quota.rs         ← QuotaChecker (moka + Timescale)
├── catalog.rs       ← PlanCatalog (seed Free/Pro/Team/Enterprise + API limits)
└── events.rs        ← UsageEvent, ActionType (AgentTurn, ApiCall, ...)
```
Implement `LagoProvider` (ensure_customer, report_usage with batched queue, webhook verifier).

**Acceptance**: Testcontainer integration test creates subscription + reports usage.

#### Phase 5 — Quota + Metering Middleware (3 AI-hours)
- Extend existing `RouterQuotaLayer` (do **not** create new file) to accept `Arc<BillingProvider> + Arc<QuotaChecker>`.
- New `mw/meter.rs` (post-handler, inserts `AgentTurnStats`).
- Add `ActionType::ApiCall`.

**Acceptance**: 51st free-tier call returns 429 with upgrade URL.

#### Phase 6 — Billing Routes + Webhook + Referrals (3 AI-hours)
- `routes/billing.rs` (protected): plans, subscription, portal, invoices, usage, **api-keys**.
- Public webhook `POST /v1/billing/webhooks` (Lago signature verification → update Zitadel claim + SSE).
- Referral endpoint + link generator (`/account/referrals`).

**Acceptance**: Webhook idempotency test passes; referral credit applied.

#### Phase 7 — Frontend UI (Usage, Invoices, API Keys, Referrals) (4 AI-hours)
- SvelteKit: `/account/billing`, `/account/usage`, `/account/invoices`, `/account/api-keys`.
- Shared `packages/ui` components: `PlanBadge`, `UsageMeter`, `PlanCard`, `ApiKeyTable`.
- SSE integration for live updates.
- Askama admin templates for super-admin billing dashboard.

**Acceptance**: Playwright E2E passes (upgrade → Pro badge live in <5 s).

#### Phase 8 — Migration, Cleanup, Ops (2 AI-hours)
- Migration script (`scripts/migrate-to-zitadel-lago.ts`).
- Flip default flag, deprecate legacy auth.
- Update docs, runbook, OpenTelemetry metrics.
- Promote ADR 012 to Accepted.

**Acceptance**: Legacy path removed; nightly reconciliation job green.

---

### 4. Configuration Surface (Env Vars)

```env
CONUSAI_AUTH_PROVIDER=zitadel|legacy
ZITADEL_DOMAIN=...
LAGO_API_URL=...
STRIPE_SECRET_KEY=...          # passed only to Lago
BILLING_RETURN_URL=...
CONUSAI_PLAN_CATALOG_PATH=/etc/conusai/plans.toml  # optional
```

---

### 5. Test Strategy & Risks (Brief)

- Unit / Integration / E2E as defined in previous plan.
- Risks & mitigations: webhook lag (SSE + poll), quota drift (moka + 30 s sync), PCI (never touch cards).

---

### 6. Effort & Token Cost Estimate

| Phase | AI-hours | Approx. token cost |
|-------|----------|--------------------|
| 0–3   | 11       | ~250k             |
| 4–6   | 11       | ~250k             |
| 7–8   | 6        | ~140k             |
| **Total** | **28** | **~640k**         |

(34 AI-hours if you also want the full code diffs generated immediately.)

---

### 7. Definition of Done

- User can sign up via Zitadel (email/social), get Free plan, hit quota → 429 + upgrade prompt.
- API key creation + usage in 3rd-party app works with correct rate limits.
- Upgrade flow → Stripe Checkout → Pro badge + higher limits live in <5 s.
- Usage/invoices page shows real-time charts + downloadable PDFs.
- Referral link gives 30 % discount + credit.
- All data lives in Zitadel + Lago; our code stays thin and extensible.

This plan removes all auth/billing debt forever and makes ConusAI the most developer-friendly agent platform in 2026.

**Next step**: Reply **“Generate the full code diffs”** and I will output every file change (Cargo.toml, identity/zitadel.rs, billing-core, middleware, routes, SvelteKit pages, docker-compose, ADR). Or say **“Start with Phase 0–2”** for the first PR.

Ready to ship?