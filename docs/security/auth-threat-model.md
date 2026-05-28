# Auth Threat Model — Epifly v5.1 OIDC

STRIDE threat model for the Zitadel/OIDC auth surfaces shipped in Plan v5.1.

---

## Surfaces in scope

| Surface | Description |
|---|---|
| Web BFF (`/auth/login`, `/auth/callback`, `/auth/logout`) | SvelteKit server-side OIDC endpoints |
| Web proxy (`/api/[...path]`) | Server-side reverse proxy that injects Bearer |
| Backend gateway (`/v1/*`) | Rust/Axum JWT verification + tenant binding |
| Native PKCE flow | Tauri 2 system-browser auth + Rust token manager |
| Keychain / session store | OS-native keychain (native) or Postgres AEAD (web) |

---

## Threat catalogue (STRIDE)

### S — Spoofing

| # | Threat | Control |
|---|---|---|
| S1 | Attacker presents a JWT signed with `alg=none` or `alg=HS256` | Gateway allowlists `{ RS256 }` only; algorithm taken from server-side allowlist, not from the token `alg` header |
| S2 | Attacker signs a JWT with the HMAC of the issuer's public key | RS256 allowlist prevents HS256 confusion; `jsonwebtoken` crate rejects mismatched alg |
| S3 | Attacker replays a captured OIDC callback URL | `state` is single-use (`consumed_at`); `nonce` binds token to session; 10-min TTL on OIDC transactions |
| S4 | Attacker redirects callback to a different `redirect_uri` | Zitadel enforces exact redirect URI match; backend validates `redirect_uri` on exchange |
| S5 | Attacker injects arbitrary tenant via `X-Tenant-ID` header | Gateway returns `400 tenant_header_forbidden` in prod; web BFF strips the header before proxying |
| S6 | Attacker forges session cookie | Cookie is opaque random 256-bit id; token is encrypted at rest in Postgres; AEAD prevents forgery |

### T — Tampering

| # | Threat | Control |
|---|---|---|
| T1 | Attacker modifies encrypted token blobs in `auth_sessions` | AES-256-GCM AEAD: IV + auth tag; any bit flip fails decryption |
| T2 | Attacker tampers with `state` or `nonce` in the OIDC transaction | `state` is the Postgres PK; `nonce` is in the row; both verified before token exchange |
| T3 | Attacker modifies JWT claims in transit | RS256 signature covers all claims; `exp`, `iss`, `aud`, `sub`, `org_id` all verified |
| T4 | Attacker replaces JWKS with a forged keyset | JWKS cache is keyed by `kid`; issuer verified against `ZITADEL_ISSUER` (exact string match); discovery validated HTTPS in prod |

### R — Repudiation

| # | Threat | Control |
|---|---|---|
| R1 | User denies performing an action | Audit log (`target: "audit"`) records `auth.login.success { iss, sub, org_id }` with IP prefix and UA class; log stream is append-only |
| R2 | Token replay after logout | Refresh token revoked at Zitadel on logout; session row `revoked_at` set; ID token used for end-session redirect |

### I — Information Disclosure

| # | Threat | Control |
|---|---|---|
| I1 | Browser reads access/refresh/ID tokens | Tokens never reach the browser; session cookie is opaque; no tokens in `localStorage`, `sessionStorage`, or IndexedDB |
| I2 | Token in server logs | Audit log never includes token bodies, `code`, `email`, full IP, `Authorization`, `Cookie`, or callback query string; `console.error` in callbacks strips token fields |
| I3 | Token in browser dev tools | Network tab shows no `Authorization` from browser; proxy injects it server-side |
| I4 | Sentry / error reporting includes tokens | Sentry `denyUrls` and `beforeSend` deny-list `authorization`, `cookie`, `set-cookie`, callback query strings, and `code`/`token` body fields |
| I5 | Native refresh token in JS | Rust token manager never returns refresh token via Tauri command; `auth:get_access_token` returns access token only |
| I6 | Secrets in production web bundle | `AUTH_SESSION_PEPPER`, `ZITADEL_GATEWAY_INTROSPECT_SECRET`, `MGMT_PAT` are server-only envs; CI asserts `rg -i "ZITADEL.*SECRET|client_secret" apps/web/.svelte-kit/output/client` → no hits |
| I7 | Cross-tenant data leak | Tenant context derived from `org_id` JWT claim via binding table; every resource query scoped to `tenant_id` |

### D — Denial of Service

| # | Threat | Control |
|---|---|---|
| D1 | Flood `/auth/login` or `/auth/callback` | Rate limiter: 10 rpm per IP (web BFF + backend gateway); `429 Too Many Requests` returned |
| D2 | Large callback bodies | `DefaultBodyLimit::max(256 KiB)` on auth routes; web proxy limits non-SSE to 25 MiB |
| D3 | OIDC transaction table bloat | Cron cleanup deletes transactions older than 1 day; session cleanup runs every 15 min |
| D4 | JWKS lookup DoS on unknown kid | Negative cache prevents repeat JWKS refresh on unknown `kid`; single-flight prevents thundering herd |

### E — Elevation of Privilege

| # | Threat | Control |
|---|---|---|
| E1 | Attacker gains admin role by forging claims | Roles extracted from signed JWT `urn:zitadel:iam:org:project:roles` claim; static mapping in `identity/roles.rs`; no client-settable role claim |
| E2 | Attacker escalates to another tenant | `tenant_id` derived from `org_id` binding, not from any header or request param; cross-tenant probe in `e2e/web/auth-zitadel.spec.ts` |
| E3 | Auto-provisioning creates unauthorized tenants in prod | `AUTH_AUTO_PROVISION_TENANTS` gated by env flag; CI asserts it's absent from prod compose files |
| E4 | Dev-auth feature active in prod release | `dev-auth` is a Cargo feature; CI build uses `--no-default-features` for prod; `#[cfg(feature = "dev-auth")]` gates all dev paths |
| E5 | Session fixation: attacker pre-sets a session id | Session id **rotated** after callback (`createSession` generates a fresh id); old id never reused |
| E6 | CSRF on logout POST | Logout route checks: same `Origin` as `HOST`, session cookie present; state cookie check for OIDC-bound endpoints |

---

## RFC 9700 (OAuth 2.0 Security BCP) checklist

| Item | Status |
|---|---|
| PKCE-S256 required | Done — S256 enforced; `plain` rejected |
| No implicit or password grant | Done — only `authorization_code` with PKCE |
| Exact `redirect_uri` match | Done — Zitadel enforces; Rust validates |
| `state` + `nonce` CSRF | Done — single-use state row + nonce binding |
| alg confusion prevention | Done — RS256 allowlist; never `none` or HS256 |
| Short-lived access tokens | Config — Zitadel access token lifetime ≤ 1h recommended |
| Refresh token rotation | Done — backend honors rotated refresh tokens; reuse detection forces logout |
| Sender-constrained tokens (DPoP) | Deferred — not in v5.1 scope |
| Token binding | Deferred |
| No secrets in native binary | Done — PKCE is public client; no `client_secret` in native |

---

## Out-of-scope / deferred

- External IdP (Google, Apple, GitHub) federation inside Zitadel — operator config
- WebAuthn / passkeys standalone (Zitadel hosts the factor)
- Multi-region HA for Zitadel
- Audit log export to SIEM
- DPoP / sender-constrained tokens for BFF→gateway
- Account deletion / GDPR export

---

_Last reviewed: Plan v5.1, Phase 7. Reviewers: security-aware list required per Phase 9._
