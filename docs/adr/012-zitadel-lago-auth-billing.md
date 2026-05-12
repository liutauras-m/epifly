# ADR 012 — Zitadel + Lago for Auth, Identity, and Billing

**Status:** Accepted

## Context

The platform currently uses HMAC session cookies and HS256 JWTs issued by the gateway itself.
Plan tiers are compile-time constants. There is no user/tenant store, no subscription billing,
no usage metering, and no OAuth2/OIDC support. This makes passwordless login, social login,
multi-tenant SaaS billing, and quota enforcement impossible without a full rewrite.

## Decision

Replace the dev HMAC/JWT auth stack with:

- **Zitadel** for OAuth2/OIDC identity management (organizations = tenants, projects = roles,
  custom claims for `plan_tier` and `subscription_status`).
- **Lago** (self-hosted) for subscription management, real-time usage metering, and invoicing.
- **Stripe** (via Lago) for payment collection. Card data never touches our process.

The existing HMAC/JWT path is preserved behind `CONUSAI_AUTH_PROVIDER=legacy` for 30 days
after cutover, then deleted.

## Architecture

```
┌──────────┐      OIDC (PKCE)     ┌──────────┐
│ Web/Tauri├─────────────────────▶│ Zitadel  │  Org=Tenant, Project=Roles
└────┬─────┘                      └────┬─────┘
     │  Bearer access_token            │  custom claims: plan_tier, sub_status
     ▼                                 ▼
┌────────────────────────────────────────────┐
│ agent-gateway (Axum)                       │
│  ├─ mw::identity    verify access_token    │
│  ├─ RouterQuotaLayer (extended)            │
│  ├─ <handler>                              │
│  └─ mw::meter       BillingProvider.report │
└──────────┬─────────────────────────────────┘
           │  REST (lago HTTP client)        ▲
           ▼                                 │  webhook (HMAC)
┌──────────────┐  Stripe (PCI)   ┌──────────┴───┐
│ Lago (self)  ├────────────────▶│ Stripe       │
│ plans+events │                  │ Checkout/Sub │
└──────┬───────┘                  └──────────────┘
       │ webhook → /v1/billing/webhooks
       ▼
   Postgres + Redis (Lago)        moka (in-process quota cache)
```

## Single sources of truth

| Concern | Owner |
|---------|-------|
| Identity / tenants / roles | Zitadel |
| Plans / subscriptions / usage / invoices | Lago |
| Payments / cards | Stripe (Lago proxies) |
| Real-time quota cache | moka (in-process) |

## Consequences

- Zitadel adds an infrastructure dependency; access tokens are cached at TTL so a Zitadel
  outage degrades gracefully.
- Lago adds Postgres + Redis; both are already required for Zitadel.
- Plan limits move out of Rust compile-time constants into a TOML catalog loaded at boot,
  enabling non-recompile plan changes.
- PCI scope is minimised: Stripe Checkout only; CSP `frame-src https://checkout.stripe.com`.
- Legacy auth path survives 30 days behind feature flag then is deleted.
