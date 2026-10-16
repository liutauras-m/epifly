# ConusAI Auth + Tenancy — Implementation Plan v0.8

> **Status (2026-10-16): historical, superseded, never implemented.** This plan proposed a
> `crates/auth-core` library issuing HS256 JWTs (`JWT_SECRET`) after OAuth logins, bridged to an
> HMAC UI session (`UI_SESSION_KEY`) and a dev name+plan login form. None of it was built, and
> every piece of code it planned to extend (HS256 tenant middleware, `routes/auth.rs`, the HMAC
> session, the `/login` page) has been deleted. The instructions it contained no longer apply and
> have been removed; the full text is in git history.

## What replaced it

Authentication is Zitadel OIDC end to end, with no platform-issued tokens:

- **Gateway** — every protected route requires `Authorization: Bearer <token>`: a Zitadel access
  token (JWT verified against the issuer's JWKS — RS256, `iss`, `aud`, `exp` — or an opaque token
  via introspection when `ZITADEL_INTROSPECTION_CLIENT_ID`/`_SECRET` are set) or a machine API key
  from `API_KEYS` (`<blake3-hex>:<tenant>:<label>`). The tenant is the user's Zitadel organization
  (`urn:zitadel:iam:user:resourceowner:id`), roles come from `urn:zitadel:iam:org:project:roles`
  (`super_admin` opens `/admin`), and the plan tier is resolved server-side from the tenant's Lago
  subscription. Code: `apps/backend/crates/agent-gateway/src/auth.rs`,
  `apps/backend/crates/agent-core/src/identity/`.
- **Web app** — a backend-for-frontend: server-side authorization code + PKCE against Zitadel,
  tokens sealed in the `__Host-epifly_session` cookie, `/v1`, `/api`, `/ui`, `/mcp` and `/admin`
  proxied to the gateway with the bearer added. See [frontend.md](frontend.md) §6.3–6.4.
- **Browser shell** — Rust-side loopback PKCE sign-in, refresh token in a Stronghold vault. See
  [frontend.md](frontend.md) §7.

References: [arch.md](arch.md) §5.3–5.4 (gateway auth), [deploy-conusai-cloud.md](deploy-conusai-cloud.md)
(Zitadel project and application setup, environment variables).
