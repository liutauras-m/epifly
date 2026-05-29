# Plan v5.1 — Zitadel/OIDC end-to-end auth (web + iOS native)

> **Goal.** Replace the dev HS256 `/v1/auth/login` with a real Zitadel/OIDC
> flow on (a) the SvelteKit web app and (b) the Tauri 2 native app on iOS
> (and macOS/Windows for free). The flow covers: register → email
> verification → sign-in → access the gateway with a Zitadel-issued JWT →
> refresh silently → sign-out everywhere.
>
> **Design principles.** Simplicity, one explicit strategy per concern,
> standards-only OAuth (RFC 9700 BCP, PKCE-S256, BFF for web, system browser
> for native), no clever shortcuts. Security posture: Swiss-bank, generic
> enough to host highly-regulated tenants on the same code path.
>
> **Anti-goals.**
> - No tokens in `localStorage` / `sessionStorage` / IndexedDB / `UserDefaults`.
> - No tokens inside a browser cookie payload (not even encrypted).
> - No client secret in any web bundle or native binary.
> - No in-app WebView for IdP pages.
> - No `email` as identity.
> - No dual verification strategy on the same route.
> - No silent **tenant** auto-creation in production.
> - No "dev mode" that can boot in production.

---

## Current-state audit (before any code change)

Verified in the repo on 2026-05-28:

| Area | Current reality | What that means for the plan |
|---|---|---|
| `apps/backend/crates/agent-core/src/identity/legacy.rs` | HS256 verifier reading `JWT_SECRET`. | Phase 9 deletes this. Until then it stays for tests behind a `cfg(feature = "dev-auth")` gate. |
| `apps/backend/crates/agent-core/src/identity/zitadel.rs` | `ZitadelProvider` does **OAuth 2.0 token introspection** (`/oauth/v2/introspect`) with `blake3`-keyed `moka` cache. **No** JWKS / local JWT verification yet. | Phase 0 adds a JWKS verifier as the **default** path and keeps the existing introspection as an opt-in for revocation-sensitive routes. The `moka` cache and the `ZitadelConfig::from_env` shape are reused. |
| `apps/backend/crates/agent-gateway/src/state.rs` | In prod profile it `require("JWT_SECRET")` and also constructs `ZitadelProvider::from_env()`. | Phase 0 drops the `JWT_SECRET` requirement when `ZITADEL_ISSUER` is configured; replaces it with a `ZITADEL_TOKEN_VERIFY_MODE` env that is one of `jwks` (default) or `introspection`. Refuses to start if both are set ambiguously. |
| `apps/backend/crates/agent-gateway/src/routes/auth.rs` | `POST /v1/auth/login` issues HS256. | Phase 0 returns `410 Gone` when Zitadel is configured; Phase 9 deletes the function and the route. |
| `apps/backend/crates/agent-gateway/src/ui/handlers/chat.rs` (line 125–128) | Has dev-fallback branch keyed on `JWT_SECRET` unset OR `CONUSAI_TEST_MODE=1`. | Phase 9 removes the `JWT_SECRET` branch; test-only path keeps `CONUSAI_TEST_MODE`. |
| `packages/sdk/src/endpoints.ts` | Exposes `AUTH_LOGIN: '/v1/auth/login'`. | Phase 2 drops the export; SDK has no opinion about login transport — it's a BFF redirect on web and a native command on iOS. |
| `packages/features/src/sdk/token-provider.ts` | `createWebTokenProvider` reads `sessionStorage["epifly.web.access_token"]`; exports `setWebAccessToken` / `clearWebAccessToken`. | Phase 2 deletes the `sessionStorage` path; `createWebTokenProvider` returns `null` (proxy injects the bearer). Native gets its own provider in Phase 5. |
| `packages/types/.../openapi.d.ts` | OpenAPI still types `/v1/auth/login`. | Regenerated in Phase 9 after deletion. |
| `apps/web/src/routes/(auth)/login/+page.svelte` | Email/password form calling `sdk.auth.login`. | Phase 3 rewrites it to render the OIDC panel; primary CTAs become `<a href="/auth/login?…">`. |
| `apps/native/src-tauri/capabilities/{default,desktop,mobile}.json` | Present but no deep-link/opener capabilities. | Phase 4 adds the minimum permissions for `deep-link` and `opener` (system handler only). |

---

## Architecture in one diagram

```
                       Zitadel (issuer, IdP)
                              ▲   ▲
   Authorization Code + PKCE  │   │  JWKS (RS256)
       (state, nonce,         │   │  /oauth/v2/keys
        exact redirect_uri)   │   │
                              │   │
   ┌──────────────────────────┴───┴────────────────────────────────────┐
   │                                                                   │
   │   apps/web (SvelteKit BFF)              apps/backend (gateway)    │
   │   ┌─────────────────────────────┐       ┌───────────────────────┐ │
   │   │ /auth/login                 │       │ verify_jwt(token)     │ │
   │   │ /auth/callback              │──────▶│  - discovery (cached) │ │
   │   │ /auth/logout                │       │  - JWKS (moka, kid)   │ │
   │   │ session_store (Postgres)    │       │  - iss/aud/exp/nbf/   │ │
   │   │   id → encrypted tokens     │       │    sub/alg allowlist  │ │
   │   │ /api/[...] reverse proxy    │       │ tenant := org_id      │ │
   │   │   header allowlist;         │       │ user    := iss + sub  │ │
   │   │   injects Bearer            │       │ introspect: opt-in    │ │
   │   └─────────────────────────────┘       └───────────────────────┘ │
   │              ▲ __Host-epifly_sid (opaque)        ▲                │
   │              │ httpOnly,secure,SameSite=Lax      │                │
   │   ┌──────────┴─────────────────┐                 │                │
   │   │ browser (zero token access)│                 │                │
   │   └────────────────────────────┘                 │                │
   │                                                  │                │
   │   apps/native (Tauri 2 — iOS / macOS / Win / And)│                │
   │   ┌─────────────────────────────┐                │                │
   │   │ Svelte UI                   │                │                │
   │   │   invoke('auth:request', …) │                │                │
   │   │              ▼              │                │                │
   │   │ Rust auth + token manager   │────────────────┘                │
   │   │   - system browser (opener) │   Bearer access_token            │
   │   │   - universal link callback │                                  │
   │   │     (custom scheme fallback)│                                  │
   │   │   - Zitadel /oauth/v2/token │                                  │
   │   │     (PKCE, direct, in Rust) │                                  │
   │   │   - OS keychain persist     │                                  │
   │   │   - proactive single-flight │                                  │
   │   │     refresh + reuse detect  │                                  │
   │   └─────────────────────────────┘                                  │
   └───────────────────────────────────────────────────────────────────┘
```

**Three crisp invariants follow:**

1. **Web:** browser holds an opaque httpOnly session id only; SvelteKit
   server holds the encrypted token set; the browser never sees `Authorization`.
2. **Native:** Rust holds the tokens in the OS keychain; the WebView never
   touches refresh tokens; access tokens are handed out per-request through
   a Tauri command; no JS module caches them.
3. **Backend:** the **default** verifier is local JWT validation via JWKS.
   Introspection is opt-in per route and gated by `ZITADEL_TOKEN_VERIFY_MODE`.
   The two paths never silently mix.

---

## Non-negotiable implementation constraints

These are load-bearing; violating any of them invalidates the security model.

- **Web refresh is single-flight per `auth_sessions.id`.** Use a Postgres row
  lock (`SELECT … FOR UPDATE`) or `pg_try_advisory_xact_lock(hashtext(id))`
  inside the refresh transaction. Re-read `access_expires_at` after acquiring
  the lock; if another request already rotated, reuse the new token.
- **Logout revokes refresh tokens** at Zitadel's `revocation_endpoint` when
  discovery exposes it. Local session/keychain cleanup must complete even if
  revocation fails (best-effort, never blocking).
- **Zitadel claim names are never assumed.** `scripts/zitadel-assert-token-shape.mjs`
  runs after bootstrap; it mints a token, decodes it, and asserts the exact
  configured org and role claim keys exist. CI fails if claim names drift.
- **AASA / `assetlinks.json` deployment is exact** (see Phase 4 for the
  byte-for-byte deployment contract). Universal Links fail silently when the
  delivery contract is wrong.
- **Cleanup jobs run on `auth_oidc_transactions` and `auth_sessions`** (see
  Phase 1 schema). Auth tables are not landfill.
- **JWT `alg` is taken from a server-side allowlist, never from the token
  header alone.** The verifier ignores the JWK `alg` field for routing
  decisions and uses only `{ RS256 }`.
- **Native deep-link handling implements both `getCurrent()` (cold-start
  URL) and `onOpenUrl()` (runtime URL).** Missing one drops callbacks
  silently in one of the two lifecycles.
- **Inbound `X-Tenant-ID` / `x-tenant-id` headers are rejected at the
  edge in production.** The web BFF strips them before proxying; the
  gateway middleware returns `400 tenant_header_forbidden` when
  `APP_ENV != "dev"`. Tenancy is derived from the JWT only. CI test
  asserts this on every release build (see Phase 7).
- **DPoP / sender-constrained tokens are explicitly deferred.** They belong
  in a future plan, not Phase 7.

---

## Library choices (2026)

| Concern | Library | Why |
|---|---|---|
| Web OIDC client | [`openid-client@^6`](https://github.com/panva/openid-client) | Certified RP; discovery, PKCE, JWKS, refresh; same author as `jose`. |
| Web JOSE | [`jose@^6`](https://github.com/panva/jose) | ID-token / userinfo verification server-side. |
| Web crypto (cookie pepper, AEAD) | Node `crypto.subtle` (no extra dep) or `libsodium-wrappers` if AEAD needed | Authenticated encryption of refresh tokens at rest in Postgres. |
| Web session storage | Postgres (already in stack) | Opaque session id → server-side row. No fat cookie. |
| Backend JWT verify | Rust crate `jsonwebtoken = "9"` (already in the workspace `Cargo.toml`) + JWKS cache in `moka` (already in `agent-core`). `alg` is taken from a server-side allowlist (`{ RS256 }`), never from the token header. | Local validation against issuer JWKS with `kid` selection and negative cache. |
| Backend introspection | `reqwest` (already used by `ZitadelProvider`) | Existing code path; reserved for revocation-sensitive routes only. |
| Native deep-link | [`tauri-plugin-deep-link@2`](https://v2.tauri.app/plugin/deep-linking/) | Custom scheme + universal/app links on iOS/Android. |
| Native system browser | [`tauri-plugin-opener@2`](https://v2.tauri.app/reference/javascript/opener/) (system handler only — never `inAppBrowser`) | Opens IdP page in Safari/Chrome, not WKWebView. |
| Native keychain | `keyring = "3"` (Rust crate, called from a Tauri command) | OS-native: macOS/iOS Keychain, Windows Credential Manager, Linux Secret Service, Android Keystore. **Stronghold is deferred** — not for OAuth tokens. |
| Native OAuth in Rust | `oauth2 = "5"` | PKCE generation + token exchange in Rust, not JS. |
| Native HTTP | `reqwest` | Already used in backend; reuse style. |
| E2E web | Playwright (in repo) | `pnpm test:e2e:web`. |
| E2E iOS | WebDriverIO + Appium (in repo, `e2e/wdio`) | Keychain-injected automated tests; one full real OAuth manual smoke per release. |
| Secret/dep scanning | `gitleaks`, `cargo audit`, `osv-scanner` for JS | CI gates (Track E). |

---

## Tracks (the dependency view)

Linear phases ship in order; five tracks run in parallel and converge at
Phase 8.

| Track | Owner concern | Phases |
|---|---|---|
| **A — IdP setup** | Zitadel project/apps/claims/test users (mgmt API). | Z, 6 |
| **B — Backend verifier** | JWKS, discovery, claim validation, tenant context, legacy removal. | 0, 6, 9 |
| **C — Web BFF** | login/callback/logout, server-side session store, reverse proxy, route guards. | 1, 2, 3 |
| **D — Native** | System browser + universal/custom link, Rust token manager, keychain, refresh rotation, logout. | 4, 5 |
| **E — Security acceptance** | Token-storage proof, CSRF/state/nonce proof, cross-tenant proof, no-secret-in-bundle proof, log-redaction proof, dep + secret scan. | 7, 8 |

---

## Prerequisite — Zitadel configuration (Track A)

Done via a one-time idempotent bootstrap script
(`scripts/zitadel-bootstrap.mjs`) so the same config is reproducible per
environment (dev / staging / prod). Service-user PAT lives in `.env.local`
only.

| # | Action | Verify |
|---|---|---|
| Z.1 | Console bootstrap admin (one-time, manual). | Admin can log in. |
| Z.2 | Service user `epifly-bootstrap` with `IAM_OWNER` + PAT. | PAT stored in `.env.local`, not committed. |
| Z.3 | Project `epifly`. | Project ID echoed. |
| Z.4 | Web app `epifly-web` (PKCE-S256, **no client secret**, redirect URIs `https://${WEB_HOST}/auth/callback`, post-logout `https://${WEB_HOST}/`). | Client ID echoed. |
| Z.5 | Native app `epifly-native` (PKCE-S256, public client). Redirect URIs in priority order: `https://auth.epifly.app/native/callback` (Universal/App Link), `epifly://auth/callback` (custom scheme fallback), `http://127.0.0.1:53682/callback` (desktop loopback). | Client ID echoed. |
| Z.6 | API app `epifly-gateway` (JWT access tokens, `aud=epifly-gateway`, project role assertion **on**). | Client ID + introspection client secret echoed. |
| Z.7 | Claim mapping: project roles → `urn:zitadel:iam:org:project:roles`; org id → `urn:zitadel:iam:user:resourceowner:id`. | A decoded test token contains both. |
| Z.8 | Passwordless policy: WebAuthn allowed (passkeys). | Login page exposes the passkey factor. |
| Z.9 | Per-env `.env`: `ZITADEL_ISSUER`, `ZITADEL_WEB_CLIENT_ID`, `ZITADEL_NATIVE_CLIENT_ID`, `ZITADEL_GATEWAY_CLIENT_ID`, `ZITADEL_GATEWAY_INTROSPECT_SECRET`, `ZITADEL_AUDIENCE=epifly-gateway`, `ZITADEL_TOKEN_VERIFY_MODE=jwks`, `ZITADEL_BOOTSTRAP_PAT`, `AUTH_REDIRECT_BASE`, `AUTH_SESSION_PEPPER=$(openssl rand -base64 48)`, `AUTH_AUTO_PROVISION_TENANTS=false` (prod) / `true` (dev). | `docker compose config --quiet` clean. |
| Z.10 | `scripts/zitadel-assert-token-shape.mjs`: provisions a throwaway user via mgmt API, mints an access token, decodes it, asserts that `iss`, `aud`, `sub`, the configured org claim (`urn:zitadel:iam:user:resourceowner:id` by default), and the configured project-roles claim (`urn:zitadel:iam:org:project:roles` by default) all exist with the exact spelling Phase 0 expects. Writes `tests/fixtures/zitadel-token-shape.json` for downstream tests. | Script exits 0 against the local Zitadel; CI runs it on every PR that touches `agent-core/identity/**` and fails on drift. |

> **Stop condition.** If Z.1–Z.10 are not all green, do not start Phase 0.

---

## Execution checklist

- [ ] **Z** — Zitadel bootstrap script + per-env `.env` + token-shape assertion fixture
- [x] **Phase 0** — Backend JWT verification via JWKS (default) + introspection opt-in
- [x] **Phase 1** — Web BFF: login + callback + logout, **opaque cookie + server-side session store**
- [x] **Phase 2** — Web `/api/[...path]` reverse proxy with header **allowlist** + SSE
- [x] **Phase 3** — Web account UI wired to real OIDC; route guards; error pages
- [x] **Phase 4** — Native: system browser + Universal/App Link first, custom scheme fallback, **direct** code-for-token exchange in Rust
- [x] **Phase 5** — Native OS-keychain storage + proactive single-flight refresh + reuse-detection logout
- [x] **Phase 6** — Tenant binding policy (`issuer + sub` → user; `org_id` → tenant); explicit provisioning policy
- [x] **Phase 7** — Hardening: CSP, audit log, secret/dep scan, no-token-in-logs
- [ ] **Phase 8** — Acceptance: web (Playwright) + iOS (keychain-injected WDIO + manual smoke)
- [x] **Phase 9** — Delete legacy `/v1/auth/login`, remove dev-auth, regenerate OpenAPI types, doc updates

---

## Phase 0 — Backend JWT verification via JWKS (Track B)

**Strategy (single rule).**
> Gateway validates Zitadel access tokens locally via issuer discovery + JWKS cache.
> Introspection is enabled only when `ZITADEL_TOKEN_VERIFY_MODE=introspection` (global) **or** a route opts in via a `RequireIntrospection` extractor. Both paths never silently mix.

**Files.**
- `apps/backend/crates/agent-core/src/identity/zitadel.rs`:
  - Add `verify_jwt(token) -> Result<IdentityContext, AuthError>`:
    - resolves discovery (cached, validates `issuer == ZITADEL_ISSUER`, endpoints HTTPS in non-dev, `id_token_signing_alg_values_supported ⊇ ["RS256"]`, fails closed on mismatch);
    - fetches JWKS (moka, 10 min global TTL on the keyset, negative cache for unknown `kid`, refresh-on-`kid`-miss with single-flight via `tokio::sync::OnceCell`). **Cache scope is the JWKS *keyset*, not individual JWTs** — per-token expiry is enforced by the `exp` claim check, so no per-entry TTL on `moka` is required. If a future need arises to cache *decoded tokens* (we do not today), enable `moka`'s `future` + per-entry expiry policy (`expire_after`) and bound entries by `min(exp - now, hard_cap)`, never by a hard-coded 60s;
    - decodes header; asserts `alg ∈ {RS256}` (allowlist); selects key by `kid`;
    - decodes claims; asserts `iss` exact, `aud` contains `ZITADEL_AUDIENCE`, `exp` valid (60s skew), `nbf`, `iat` skew, `sub` non-empty;
    - extracts `org_id` (Z.7 mapping) and project roles; rejects if missing.
  - Add `enum VerifyMode { Jwks, Introspection }` on `ZitadelConfig`; default `Jwks`.
  - Keep `verify_access_token` (introspection) as today; route it from `IdentityProvider::verify_access_token` when `VerifyMode::Introspection` is configured.
- `apps/backend/crates/agent-gateway/src/state.rs`:
  - Drop `require("JWT_SECRET")` when `ZITADEL_ISSUER` is configured. Replace with `require("ZITADEL_ISSUER")` and the appropriate verify-mode envs.
  - Refuse to start if `ZITADEL_TOKEN_VERIFY_MODE` is unset *and* both JWKS and introspection envs are present (ambiguity error).
- `apps/backend/crates/agent-gateway/src/mw/tenant.rs`:
  - Use `verify_jwt` for the default path; keep an opt-in `RequireIntrospection` extractor for revocation-sensitive routes (e.g. session-revoke webhook).
- `apps/backend/crates/agent-gateway/src/routes/auth.rs`:
  - When `ZITADEL_ISSUER` is set, `POST /v1/auth/login` returns `410 Gone` with `error_code: "use_oidc"`.

**Tests (write first).**
- `cargo test -p agent-core zitadel::jwks::accepts_valid_jwt` (mock JWKS, real RSA key, real JWT).
- Negatives: expired, wrong `iss`, wrong `aud`, wrong `alg` (HS256 attack: signed RS256 token re-signed with HS256 must be rejected — `alg=none` and `alg=HS256` both rejected), missing `kid`, unknown `kid` (one refresh, then reject), missing `sub`, missing `org_id`.
- `unknown_kid_triggers_single_refresh` — N concurrent requests cause exactly **one** JWKS refresh.
- `verify_mode_ambiguity_fails_startup` — both modes configured → state ctor returns error.

**Verify (web).**
1. Boot stack with Zitadel.
2. Mint a real access token by completing a manual OAuth flow with a mgmt-API-provisioned user (`scripts/zitadel-test-token.sh user@test.epifly`).
3. `curl -fsS :8080/v1/workspaces/tree -H "Authorization: Bearer $TOKEN"` → `200`.
4. `curl -i :8080/v1/workspaces/tree -H "Authorization: Bearer not.a.token"` → `401`.
5. `curl -i :8080/v1/auth/login -X POST -d '{}'` → `410`.

**Verify (iOS).** N/A. Sanity: existing iOS app still boots.

**Reviewer checklist.**
- [x] One default verification strategy (JWKS); introspection only via explicit opt-in.
- [x] `alg` allowlisted (no `none`, no HS256 confusion).
- [x] `iss` exact-string equality.
- [x] JWKS cache single-flight on miss + negative cache.
- [x] No token, code, or claims body in any log call in this phase.
- [x] `cargo clippy -p agent-gateway -p agent-core --all-targets -- -D warnings` clean.

---

## Phase 1 — Web BFF: login / callback / logout, opaque cookie + server-side session store (Track C)

**Files.**
- `apps/web/src/lib/server/auth/oidc.ts` — `openid-client` config + `discover()` with the same strict validation as Phase 0 (issuer, HTTPS in prod, alg allowlist, discovery host allowlist). Fails closed on mismatch.
- `apps/web/src/lib/server/auth/session.ts` — opaque session id (`crypto.randomUUID()` + 256-bit suffix); Postgres-backed store. Access/refresh/id tokens are encrypted at rest using `AUTH_SESSION_PEPPER` (libsodium `crypto_secretbox` or Node `aes-256-gcm` with random nonce).
- `apps/web/src/routes/auth/login/+server.ts` — generates PKCE verifier, state, nonce; persists them in a separate short-lived **transaction row** keyed by `state` (replay-detected via `consumed_at`); sets a tiny `__Host-epifly_oidc_tx` cookie pointing to the row id; validates `returnTo` against an allowlist; 302 to authorize endpoint.
- `apps/web/src/routes/auth/callback/+server.ts` — validates state/nonce against the tx row; marks it consumed (double-callback → `400`); exchanges code (PKCE); verifies ID token (`iss`, `aud`, `nonce`, `exp`); creates session row; **rotates session id**; sets `__Host-epifly_sid` (httpOnly, secure, sameSite=lax, path=/, no Domain, rolling max-age).
- `apps/web/src/routes/auth/logout/+server.ts` — (1) calls Zitadel `revocation_endpoint` for the refresh token if discovery exposes it (best-effort, 2s timeout, failure is logged but does not block); (2) marks the session row `revoked_at = now()`; (3) clears `__Host-epifly_sid` and `__Host-epifly_oidc_tx`; (4) redirects to `end_session_endpoint` with `id_token_hint` and `post_logout_redirect_uri`. The end-session redirect ends the **IdP browser session** and is not a substitute for refresh-token revocation.
- `apps/web/src/hooks.server.ts` — reads `__Host-epifly_sid`, loads session, attaches `event.locals.session = { userIss, userSub, tenantOrgId, displayName, emailVerified }`. Refresh path is **single-flight per `auth_sessions.id`**:
  ```
  BEGIN;
    SELECT access_ct, refresh_ct, access_expires_at
      FROM auth_sessions WHERE id = $1 FOR UPDATE;
    -- re-check; another concurrent request may have already rotated
    if access_expires_at >= now() + 60s: COMMIT; return existing token;
    -- otherwise call Zitadel /oauth/v2/token with grant_type=refresh_token
    UPDATE auth_sessions
       SET access_ct = $new_access_ct,
           refresh_ct = $new_refresh_ct,        -- rotated
           access_expires_at = $new_exp,
           last_seen_at = now()
     WHERE id = $1;
  COMMIT;
  ```
  On `invalid_grant`: mark `revoked_at`, clear cookie, redirect to `/auth/login?reason=expired_session`. Five concurrent in-flight requests after expiry must produce exactly **one** refresh round-trip (verified by integration test).

**Schema (one migration).**
```sql
CREATE TABLE auth_sessions (
  id                text PRIMARY KEY,        -- opaque, 256-bit
  user_iss          text NOT NULL,
  user_sub          text NOT NULL,
  tenant_org_id     text NOT NULL,
  access_ct         bytea NOT NULL,          -- AEAD(access_token)
  refresh_ct        bytea NOT NULL,          -- AEAD(refresh_token)
  id_token_ct       bytea,                   -- kept only when needed for end_session id_token_hint; nulled out after first /auth/logout call
  access_expires_at timestamptz NOT NULL,
  created_at        timestamptz NOT NULL DEFAULT now(),
  last_seen_at      timestamptz NOT NULL DEFAULT now(),
  revoked_at        timestamptz
);
CREATE INDEX auth_sessions_user ON auth_sessions(user_iss, user_sub);

CREATE TABLE auth_oidc_transactions (
  state         text PRIMARY KEY,
  code_verifier text NOT NULL,
  nonce         text NOT NULL,
  return_to     text NOT NULL,
  created_at    timestamptz NOT NULL DEFAULT now(),
  consumed_at   timestamptz
);
```

**Cleanup jobs** (cron task in the SvelteKit server, runs every 15 min):

- `DELETE FROM auth_oidc_transactions WHERE created_at < now() - interval '1 day';`
- `DELETE FROM auth_sessions WHERE revoked_at IS NOT NULL AND revoked_at < now() - interval '7 days';`
- `DELETE FROM auth_sessions WHERE last_seen_at < now() - interval '30 days';` (idle expiry)
- `DELETE FROM auth_sessions WHERE created_at < now() - interval '90 days';` (hard max lifetime, regardless of activity)

All cleanups are batched (`LIMIT 1000`) and logged with row counts only.

> **Hard rule.** The session cookie carries an opaque session id and nothing
> else. No `access_token`, no `refresh_token`, no `id_token`, no `sub`, no
> `email`. Token bodies live only in `auth_sessions.*_ct` (encrypted) and are
> decrypted in memory per request inside the BFF.

**Verify (web).**
1. `pnpm --filter web dev`.
2. `/auth/login?returnTo=%2F` → Zitadel.
3. Sign in as a mgmt-API-provisioned user (Phase 8).
4. Land on `/`. `__Host-epifly_sid` present and `HttpOnly`; `document.cookie` does not contain it.
5. Row in `auth_sessions`; `access_ct` is binary (not JWT-decodable).
6. Replay the callback URL → `400 transaction_already_consumed`.
7. `/auth/login?returnTo=https://evil.example` → `400`.
8. `/auth/logout` → cookie cleared; row `revoked_at IS NOT NULL`; Zitadel session ended.

**Verify (iOS).** N/A.

**Reviewer checklist.**
- [x] Cookie payload is opaque only; tokens never reach the browser.
- [x] Tokens encrypted at rest with authenticated encryption (AEAD).
- [x] `state` stored server-side; one-time use enforced by `consumed_at`.
- [x] `returnTo` allowlisted server-side.
- [x] Session id rotated after callback (fixation prevention).
- [x] PKCE S256 only; no implicit / hybrid / password grants.
- [x] Discovery validated and cached; fails closed on mismatch.

---

## Phase 2 — Web `/api/[...path]` reverse proxy with header allowlist + SSE (Track C)

**Files.**
- `apps/web/src/routes/api/[...path]/+server.ts` — proxy with **header allowlist**:
  - **Forward client → backend:** `accept`, `content-type`, `accept-language`, `cache-control` (limited values), `x-request-id` (generate if absent).
  - **Drop unconditionally:** `cookie`, `authorization`, `host`, `connection`, `upgrade` (except SSE), `x-forwarded-*`, `content-length` (recomputed), any `x-internal-*`.
  - **Inject:** `Authorization: Bearer ${decrypted_access}` from `event.locals.session`. The proxy never sets `x-tenant-id` — the backend derives tenancy from the JWT.
  - **Path normalization:** allowed upstream prefixes are `/v1/` and `/healthz` (only when explicitly unauthenticated). Reject anything containing `..`, double slashes, or that escapes the allowlist after normalization.
  - **Method allowlist:** `GET`, `POST`, `PUT`, `PATCH`, `DELETE`.
  - **Body size limit:** `MAX_PROXY_BODY = 25 MiB` for non-SSE; streaming for SSE.
  - **Timeout:** 60s default, 5 min for SSE; honors `event.request.signal` so client disconnect cancels upstream within ≤500 ms.
- `apps/web/src/routes/+layout.svelte` — SDK `baseUrl: '/api'`.
- `packages/features/src/sdk/token-provider.ts` — replace `createWebTokenProvider`'s `sessionStorage` body with `() => null`. Remove `setWebAccessToken` / `clearWebAccessToken` exports. Update `packages/features/src/index.ts` re-exports.
- `packages/sdk/src/endpoints.ts` — drop `AUTH_LOGIN`; SDK loses any opinion about login transport.

**Verify (web).**
1. DevTools → Network: no request from the browser hits `:8080`; all go to `/api/...` on `:5173`.
2. None of those carry `Authorization` from the client.
3. `rg -nE "localStorage|sessionStorage" apps/web/src packages/features/src | rg -iE "token|access|refresh"` → no hits.
4. SSE: `POST /api/v1/chat/stream` works end-to-end; aborting the page cancels upstream within 500 ms (gateway log).
5. Path traversal: `GET /api/v1/../../internal/foo` → `400`.
6. Body limit: `POST /api/v1/files` with 100 MiB → `413`.

**Reviewer checklist.**
- [x] Allowlist, not blocklist.
- [x] Proxy never reads client `authorization` header.
- [x] SSE preserves `text/event-stream` and disables buffering.
- [x] Cancellation propagates; verified by integration test.
- [x] `setWebAccessToken` and the sessionStorage key are gone from the codebase.

---

## Phase 3 — Web account UI wired to real OIDC (Track C)

**Files.**
- `packages/ui/src/components/account/auth-onboarding-panel.svelte` — drop email/password fields. Props: `{ mode: 'signin' | 'register', returnTo, idps, isSubmitting, error }`. CTAs are anchors to `/auth/login?...`.
- `apps/web/src/routes/(auth)/login/+page.svelte` — renders the panel; no client-side login call.
- `apps/web/src/routes/(auth)/login/+page.server.ts` — if already authenticated, redirect to `returnTo` (allowlisted).
- `apps/web/src/routes/(app)/+layout.server.ts` — `if (!locals.session) throw redirect(302, '/auth/login?returnTo=' + url.pathname);`
- `apps/web/src/routes/auth/error/+page.svelte` — handles: expired transaction, email not verified, org not allowlisted, tenant not provisioned, user cancelled, generic OIDC error. Each surfaces a single sentence + retry CTA.

**IdP buttons.** Google/Apple/GitHub buttons render **only if** Zitadel has the IdP configured (read from `GET /v1/auth/idps`). No "Soon" placeholders.

**Verify (web).**
1. `/` → `/auth/login?returnTo=%2F`.
2. Click "Sign up for free" → Zitadel register page.
3. After register + verify, land back on `/`.
4. Sidebar shows display name (from `name` / `preferred_username` claim — **not** email; email is shown only as secondary contact when `email_verified == true`).
5. Sign out → cookies cleared.
6. Trigger each error path; the error page renders correct copy.

**Reviewer checklist.**
- [x] No email/password field rendered anywhere.
- [x] Route guard runs server-side (`+layout.server.ts`), not in a client `$effect`.
- [x] IdP buttons reflect actual backend config.
- [x] Error page covers the listed OIDC failure modes.

---

## Phase 4 — Native: system browser + Universal Link first + direct exchange (Track D)

**Decision: Option A — native exchanges directly with Zitadel.** Backend's
role for native is **only** to verify access tokens. A half-BFF for native
returns tokens to the device anyway and adds bespoke code with no security
gain.

**Redirect priority.**
1. **Universal Link / App Link (production default):** `https://auth.epifly.app/native/callback`.
2. **Custom scheme fallback (simulator / dev):** `epifly://auth/callback`.
3. **Desktop loopback (macOS / Windows / Linux):** `http://127.0.0.1:{ephemeral}/callback`.

**Files.**
- `apps/native/src-tauri/Cargo.toml` — add `tauri-plugin-deep-link`, `tauri-plugin-opener`, `reqwest`, `keyring = "3"`, `oauth2 = "5"`, `dashmap`, `tokio`.
- `apps/native/src-tauri/tauri.conf.json` — `identifier = "app.epifly.client"`; register URL scheme `epifly`; configure associated domains for Universal Links: `applinks:auth.epifly.app`.
- `apps/native/src-tauri/capabilities/mobile.json` — allow only:
  - `deep-link:default`
  - `opener:allow-open-url` (system handler; **never `inAppBrowser`**)
  - custom commands `auth:start`, `auth:get_access_token`, `auth:sign_out`
  - **forbidden:** `shell:allow-execute`, any in-app browser permission.
- `apps/native/src-tauri/capabilities/desktop.json` — same plus loopback redirect handler.
- `apps/native/src-tauri/src/auth/mod.rs` (new):
  - `start_login(prompt) -> Url`: generates PKCE (S256), state, nonce; stores them in `DashMap<state, Transaction { code_verifier, nonce, redirect_uri, created_at }>` with TTL 10 min, capacity 16; returns authorize URL.
  - Deep-link handler: validates **scheme + host + path + state + redirect_uri exact match + transaction age**; consumes the transaction (single-use); performs token exchange against Zitadel `/oauth/v2/token` via `oauth2` crate; persists token bundle via keychain (Phase 5); emits `auth:signed_in` Tauri event.
  - `get_access_token()`: refreshes first if `expires_at < now + 60s`; returns the access token; **never** exposes the refresh token to JS.
- `apps/native/src/lib/native/auth.ts` — thin JS wrapper that invokes the Rust commands. **No JS-side token state.**
- `packages/features/src/sdk/token-provider.ts` — `createNativeTokenProvider({ invoke })` calls `invoke('auth:get_access_token')` per request.

**Universal/App-Link delivery contract (web side).** Phase 4 ships these
endpoints from the web app *exactly* as follows. Any deviation breaks the
production redirect priority silently.

*Apple AASA*
- URL: `https://auth.epifly.app/.well-known/apple-app-site-association`
- Content-Type: `application/json`
- **No redirect** (must be 200, served directly from `auth.epifly.app`).
- **No `.json` file extension** in the URL path.
- Body includes one `applinks.details` entry with `appID = TEAM_ID.app.epifly.client` and `paths = ["/native/callback*"]`.

*Android assetlinks*
- URL: `https://auth.epifly.app/.well-known/assetlinks.json`
- Content-Type: `application/json`
- Body includes `package_name = app.epifly.client` and the SHA-256 cert fingerprint for the signing key (per build variant; debug + release each get an entry).

*Phase 4 verifies both with a CI check that curls the production-style host and asserts MIME + 200 + body shape; missing the check is a release blocker.*

**Deep-link lifecycle.** The Rust handler must subscribe via **both** APIs to
cover the two lifecycles Tauri exposes:

- `get_current()` at app startup — handles the case where iOS launched the app from a cold state with the callback URL.
- `on_open_url()` at runtime — handles the case where the app was already running.

A single subscription is not enough. The cold-start path is the one most
likely to be silently dropped if forgotten.

**Verify (iOS).**
1. `cd apps/native && pnpm tauri ios dev "iPhone 16 Pro"`.
2. Tap "Sign in" → Safari opens Zitadel (verify: not WKWebView).
3. Provisioned test user (mgmt API); complete sign-in.
4. iOS returns via Universal Link → app lands on chat home; gateway call carries `Authorization: Bearer …`.
5. **Replay**: re-open the deep link URL → app rejects (`state already consumed`).
6. **Tampered state / scheme / host / path**: app rejects.
7. **Hijack test**: install a second sim app that registers `epifly://` → Universal Link still wins because associated-domains is verified.
8. **Custom-scheme fallback path** (toggle associated-domains entitlement off in a dev build): callback returns via `epifly://`; same validation applies.

**Verify (web).** N/A. Sanity: web flow still green; `assetlinks.json` and AASA served with correct MIME.

**Reviewer checklist.**
- [x] System browser only; never `inAppBrowser` for IdP pages.
- [x] Universal/App Links preferred; custom scheme is fallback only.
- [x] Deep-link handler validates scheme + host + path + state + `redirect_uri` exact match + transaction age.
- [x] State consumed exactly once.
- [x] No client secret in native binary (`strings <binary> | rg -i secret` → no hits).
- [x] Token exchange runs in Rust; JS never has the refresh token.
- [x] `redirect_uri` exactly matches one of the registered URIs (byte-for-byte).

---

## Phase 5 — Native OS-keychain storage + refresh rotation (Track D)

**Files.**
- `apps/native/src-tauri/src/auth/store.rs` (new) — **split keychain entries** (preferred): `keyring::Entry::new("app.epifly.client", "session_meta")` for `{ iss, sub, org_id, expires_at }`, `…"access_token"`, `…"refresh_token"`. Split enables targeted deletion, cleaner rotation, and moving the access token to memory-only later. A single JSON blob is allowed for MVP only if (1) it is never logged, (2) only Rust reads/writes it, (3) writes are atomic (single `set_password`), (4) keychain service/account names are stable, (5) an integration test proves logout deletes it.
- `apps/native/src-tauri/src/auth/refresh.rs` — single-flight refresh using `tokio::sync::Mutex` + `Notify`. Concurrent `get_access_token()` calls share **one** refresh round-trip.

**Behavior contract.**
- Refresh is **proactive** (60s before expiry), not only reactive on 401.
- 401 from gateway triggers exactly one forced refresh+retry, then logout.
- **Refresh-token rotation** is honored: if Zitadel rotates, the previous refresh is invalidated; the new one is persisted atomically before the in-flight request returns.
- **Reuse detection**: `invalid_grant` on a refresh → treated as a stolen-token signal → call Zitadel `revocation_endpoint` for the refresh token (best-effort, 2s timeout), wipe keychain, emit `auth:signed_out`, force re-login. No retry storm.
- **`sign_out()`** (user-initiated): call `revocation_endpoint` for the refresh token (best-effort), then wipe the keychain entry **even if revocation fails**, then emit `auth:signed_out`.
- Refresh response without a new refresh token (Zitadel without rotation) keeps the old one.
- All keychain writes are atomic; never partial.

**Verify (iOS).**
1. Sign in (Phase 4).
2. Send a chat message → succeeds.
3. Cold launch → app lands directly in chat.
4. Force short token: Zitadel access-token lifetime 60s; wait 70s; send a message → exactly one `POST /oauth/v2/token` (refresh), then `200` on the gateway call.
5. Concurrent refresh: 5 simultaneous `getAccessToken` calls right after expiry → exactly one refresh request.
6. Reuse detection: snapshot keychain blob, sign in again (rotates), restore the snapshot, make a request → app signs out cleanly, lands on login.
7. Sign out → `xcrun simctl spawn booted security find-generic-password -s 'app.epifly.client' 2>&1 | rg 'item could not be found'`.

**Reviewer checklist.**
- [x] Refresh is proactive + 401-fallback (one retry).
- [x] Single-flight by construction (mutex + notify), not best-effort.
- [x] Reuse detection wipes and signs out.
- [x] Keychain write is atomic.
- [x] Token never written to `UserDefaults`, log, or disk file.

---

## Phase 6 — Tenant + role mapping with explicit provisioning policy (Tracks A + B)

**Identity model.**
- **User identity** = `(issuer, sub)`. Primary key everywhere.
- **Tenant identity** = `tenant_identity_bindings.lookup(zitadel_issuer, zitadel_org_id)`.
- **Email** = display / contact attribute only; never identity; only trusted if `email_verified == true`.
- **Display** = `name` / `preferred_username` claim.

**Provisioning policy (explicit).**
> On first login:
> - If `(iss, org_id)` exists in `tenant_identity_bindings` → allow.
> - If no binding exists and `AUTH_AUTO_PROVISION_TENANTS=true` (dev / staging only) → create binding using `sub` as `created_by_sub`, `plan_tier=free`.
> - If no binding exists in production → reject with `403 tenant_not_provisioned`; the web error page handles it and routes to onboarding/admin approval.
>
> Silent **user-projection sync** after a verified login is allowed.
> Silent **tenant creation** in production is forbidden.

**Files.**
- `apps/backend/crates/agent-core/src/identity/binding.rs` (new):
  ```sql
  CREATE TABLE tenant_identity_bindings (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id       text NOT NULL UNIQUE,
    zitadel_issuer  text NOT NULL,
    zitadel_org_id  text NOT NULL,
    plan_tier       text NOT NULL DEFAULT 'free',
    status          text NOT NULL DEFAULT 'active',  -- active | suspended | pending
    created_by_sub  text NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    UNIQUE (zitadel_issuer, zitadel_org_id)
  );
  ```
- `apps/backend/crates/agent-gateway/src/mw/tenant.rs` — after `verify_jwt`, look up binding; on miss apply the policy above.
- `apps/backend/crates/agent-core/src/identity/roles.rs` — static, versioned mapping:
  ```rust
  // version 1
  "tenant.admin"   => AppRole::TenantAdmin,
  "tenant.member"  => AppRole::TenantMember,
  "platform.admin" => AppRole::PlatformAdmin,
  ```
  Cached per token (TTL = token expiry).
- CI gate: `AUTH_AUTO_PROVISION_TENANTS=true` must not appear in any `dokploy/**` or prod compose file.

**Verify (web + iOS).**
1. Provision two orgs `tenant-a`, `tenant-b`, two users `a@…`, `b@…`. Both sign in on web and iOS.
2. From a `tenant-a` session, `GET /api/v1/workspaces/{tenant-b-node-id}` → `404` (not `200`, not `500`).
3. User with no org claim → `403 tenant_not_provisioned`.
4. User with org but no project role → minimal role; can read profile, cannot create workspaces.
5. Force role downgrade in Zitadel → effective at next token refresh (≤ access-token lifetime).
6. SQL: `SELECT * FROM tenant_identity_bindings WHERE zitadel_org_id = $1` returns exactly one row; never derived from email.
7. Acceptance SQL probes everywhere use `WHERE issuer = $1 AND subject = $2`, not `email`.

**Reviewer checklist.**
- [x] `email` never appears in `tenant_identity_bindings`.
- [x] Auto-provision gated by an env flag that cannot be true in prod (CI enforced).
- [x] Frontend never sends `x-tenant-id`; backend derives it from the JWT.
- [ ] Cross-tenant probe is in `e2e/web/auth-zitadel.spec.ts`.

---

## Phase 7 — Hardening (Track E)

**Implementation.**
- **CSP** on the web app: `default-src 'self'; script-src 'self'; connect-src 'self' https://${ZITADEL_HOST}; frame-ancestors 'none'; object-src 'none'; base-uri 'self'`.
- **`X-Frame-Options: DENY`**, **`Referrer-Policy: strict-origin-when-cross-origin`**, **`Permissions-Policy`** locked down, **`Strict-Transport-Security`** (prod).
- **Rate limiting** via `tower_governor` on `/v1/auth/*`, `/auth/login`, `/auth/callback`; separate buckets by IP and by transaction id (state); 10 rpm baseline.
- **Audit log** (structured, separate stream from app log):
  - `auth.login.success { iss, sub, org_id, idp, ip_prefix, ua_class }`
  - `auth.login.failure { reason, ip_prefix, ua_class }`
  - `auth.logout { iss, sub }`
  - `auth.refresh.failure { reason }`
  - `auth.tenant_binding.failure { iss, org_id, reason }`
  - **Never** contains `code`, `access_token`, `refresh_token`, `id_token`, `email`, raw claims, full IP, `Authorization`, `Cookie`, or callback query string.
- **CSRF wording.** Callback CSRF = `state` + `nonce` + consumed-once tx row. `Origin`/`Referer` on callback are **not** used (IdPs may omit them). First-party POSTs (`/auth/logout`) get `Origin` + `state` cookie checks.
- **Sentry/error reporting** deny-lists `authorization`, `cookie`, `set-cookie`, callback query strings, and any `code` / `token` body fields.
- **Secret scanning**: `gitleaks` pre-commit + CI.
- **Dep scanning**: `cargo audit` + `osv-scanner` (JS) on CI; fail on `high`.
- **Threat model**: `docs/security/auth-threat-model.md` (STRIDE per surface; explicitly addresses RFC 9700 items: exact redirect URI, PKCE, no implicit, no password grant, alg confusion, IdP page in system browser only, token replay).
- **Dev-auth lock**: Rust `dev-auth` feature; web `process.env.NODE_ENV !== 'production'`; CI fails if a release build enables `dev-auth` or if `AUTH_AUTO_PROVISION_TENANTS=true` ships in a prod profile.
- **Tenant-header rejection (production):** the gateway middleware (`mw/tenant.rs`) rejects any inbound `X-Tenant-ID` / `x-tenant-id` header with `400 { error_code: "tenant_header_forbidden" }` when `APP_ENV != "dev"`, **regardless** of whether the value matches the JWT-derived tenant. Dev mode logs a warning when the header is honored. The web BFF strips the header on every proxied request as defence-in-depth. Integration test `tenant_header_rejected_in_prod` asserts both behaviours and runs on every CI build.

**CI commands (exact, copy-pasteable — interpretation is not a security control).**
```sh
cargo clippy --workspace --all-targets -- -D warnings
cargo audit --deny warnings
osv-scanner --lockfile pnpm-lock.yaml
gitleaks detect --source . --redact --verbose
pnpm -r check-types
pnpm -r test
pnpm test:e2e:web
node scripts/zitadel-assert-token-shape.mjs        # Z.10 fixture must still match
node scripts/assert-no-auto-provision-in-prod.mjs  # AUTH_AUTO_PROVISION_TENANTS=true must not ship to prod profiles
node scripts/assert-aasa-and-assetlinks.mjs        # AASA + assetlinks served correctly from auth.epifly.app
```

**Verify (web).**
1. `curl -i :5173/` → all security headers present.
2. `/auth/callback?code=x&state=fake` from another origin → `400`.
3. 30 `/auth/login` from one IP → `429`.
4. `rg -nE "access_token|id_token|refresh_token|\\bcode=\\b" apps/backend/crates --type rust | rg -v 'test|//'` → no logging hits.
5. `pnpm --filter web build && rg -i "ZITADEL.*SECRET|client_secret|MGMT_PAT|BOOTSTRAP_PAT" apps/web/.svelte-kit/output/client` → no hits.

**Verify (iOS).** Charles trace: Zitadel pages never load inside the Tauri WKWebView.

**Reviewer checklist.**
- [x] All headers present in prod build.
- [x] Audit log schema matches list above; nothing extra.
- [x] Threat model checked in and reviewed.
- [x] CI gates: `cargo audit`, `osv-scanner`, `gitleaks`, "no dev-auth in release", "no auto-provision in prod profile".
- [x] `tenant_header_rejected_in_prod` integration test green; manual `curl -H 'X-Tenant-ID: tenant-b'` against prod profile → `400 tenant_header_forbidden`.

---

## Phase 8 — Acceptance: closed loop on web AND iOS (Track E)

**Test users are provisioned via the Zitadel mgmt API**
(`scripts/zitadel-test-user.mjs`), not via brittle UI registration. **One**
UI registration smoke per release remains as a manual gate.

### Web acceptance — `e2e/web/auth-zitadel.spec.ts`

- [ ] **8.W.1** `/` → `/auth/login?returnTo=%2F`.
- [ ] **8.W.2** Click "Sign in" → Zitadel login page.
- [ ] **8.W.3** Sign in as mgmt-API-provisioned `accept-web-{ts}` (`email_verified=true`).
- [ ] **8.W.4** Land on `/`; sidebar shows display name.
- [ ] **8.W.5** Open chat; stream a message; receive deltas.
- [ ] **8.W.6** Reload → still signed in.
- [ ] **8.W.7** Sign out → cookies cleared; protected route 302 to login.
- [ ] **8.W.8** Sign back in → same workspace tree.
- [ ] **8.W.9** `SELECT 1 FROM auth_sessions WHERE user_iss=$1 AND user_sub=$2 AND revoked_at IS NULL` → 1 row.
- [ ] **8.W.10** Cross-tenant probe (Phase 6 #2) returns `404`.

### iOS acceptance — `e2e/wdio/specs/ios/auth-zitadel.spec.ts`

Automated (keychain-injected, deterministic, runs per PR):

- [ ] **8.I.1** Cold launch with empty keychain → login screen.
- [ ] **8.I.2** Cold launch with injected valid bundle → chat home.
- [ ] **8.I.3** API call succeeds with bearer.
- [ ] **8.I.4** Force expire + invoke `get_access_token` → exactly one refresh.
- [ ] **8.I.5** Sign out → keychain entry absent.

Manual smoke (one per release, full real OAuth):

- [ ] **8.I.M.1** Tap Sign in → Safari → Zitadel → Universal-Link return → chat home.
- [ ] **8.I.M.2** Background + foreground → still signed in.
- [ ] **8.I.M.3** Kill + relaunch → still signed in.

### Cross-platform invariants

- [ ] **8.X.1** Web cookie `__Host-epifly_sid` is httpOnly + secure + sameSite=lax + path=/, no Domain attribute.
- [ ] **8.X.2** No token visible in web `localStorage` / `sessionStorage` / IndexedDB (Playwright assertion).
- [ ] **8.X.3** iOS Keychain entry present after login; absent after logout.
- [ ] **8.X.4** Gateway logs show `auth.login.success` for both runs; no token body anywhere.
- [ ] **8.X.5** `rg -RIn "JWT_SECRET|DEV_PASSWORD" --type rust --type ts | rg -v 'docs/'` → only historical migrations.

---

## Phase 9 — Decommission legacy + docs (Track B)

- [x] **9.1** Delete `agent-gateway/src/routes/auth.rs::login` and the HS256 issuance code path.
- [x] **9.2** Delete `agent-core/src/identity/legacy.rs`; remove `LegacyIdentityProvider` from `agent-gateway/src/state.rs`. Drop `JWT_SECRET` and `DEV_PASSWORD` from source.
- [x] **9.3** Delete the `JWT_SECRET` branch in `agent-gateway/src/ui/handlers/chat.rs`; keep the `CONUSAI_TEST_MODE` branch.
- [x] **9.4** Remove `setWebAccessToken` / `clearWebAccessToken` exports and the `sessionStorage` bridge (done structurally in Phase 2).
- [x] **9.5** Removed `/v1/auth/login`, `LoginRequest`, `LoginResponse`, and `login` operation from `packages/types/.../openapi.d.ts`.
- [x] **9.6** Updated `docs/arch.md` env table — dropped `JWT_SECRET` / `DEV_PASSWORD` / `CONUSAI_AUTH_PROVIDER`; added `ZITADEL_ISSUER`, `ZITADEL_TOKEN_VERIFY_MODE`; updated identity provider description.
- [x] **9.7** `CLAUDE.md` invariant #12 already reflected the implemented strategy (BFF cookie on web, OS keychain on native).
- [x] **9.8** SDK source has no legacy auth references. `packages/sdk/README.md` has no legacy references.
- [ ] **9.9** Tag the PR; require two reviewers; one must be from a security-aware reviewer list.

---

## Final reviewer checklist (cumulative — copy into the PR description)

- [ ] OIDC discovery `issuer` exactly matches configured `ZITADEL_ISSUER`; endpoints HTTPS in prod; fails closed on mismatch.
- [ ] Authorization callback rejects reused / missing / expired / tampered state; transaction row consumed exactly once.
- [ ] Callback `returnTo` allowlisted; no open redirect.
- [ ] Gateway validates `iss`, `aud`, `exp`, `nbf`, `iat` skew, `sub`, `alg` allowlist, `kid`.
- [ ] Unknown `kid` triggers exactly one JWKS refresh (single-flight); negative cache on miss.
- [ ] Access-token verification mode is explicit: `jwks` (default) or `introspection` (opt-in). Never both silently.
- [ ] Browser session cookie contains only the opaque session id.
- [ ] Refresh token is never stored in browser-readable storage or browser cookie payload.
- [ ] Refresh-token rotation is atomic and single-flight.
- [ ] `invalid_grant` clears session / keychain and forces login.
- [ ] Native OAuth opens only the system browser; never WKWebView / in-app browser.
- [ ] Native callback validates scheme + host + path + state + `redirect_uri` exact match + transaction age.
- [ ] Universal/App Links are production default; custom scheme is fallback.
- [ ] User identity is `issuer + sub`; never `email`.
- [ ] Tenant identity comes from `org_id` mapping; never `email` / domain guess.
- [ ] Production refuses unknown `org_id` unless an explicit onboarding flow exists.
- [ ] Auth logs redact `code`, tokens, cookies, `email`, and raw claims.
- [ ] Sentry / error reporting strips `Authorization`, `Cookie`, and callback query params.
- [ ] Dev auth cannot start in a production profile (CI gate).
- [ ] `AUTH_AUTO_PROVISION_TENANTS=true` cannot ship to a production profile (CI gate).
- [ ] Header proxy uses **allowlist** (not blocklist); body size + path normalization in place.
- [ ] SSE proxy preserves `text/event-stream` and propagates cancellation.
- [ ] Cross-tenant isolation probe is in the suite and green.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] `cargo audit` clean (no high+).
- [ ] `osv-scanner` clean (no high+).
- [ ] `gitleaks` clean.
- [ ] `pnpm -r check-types` and `pnpm -r test` clean.
- [ ] `pnpm test:e2e:web` green.
- [ ] iOS acceptance (automated + manual smoke) green for the release tag.

---

## Iteration protocol for the implementing agent

```
loop:
  1. Pick next unchecked step from the Execution checklist.
  2. State the contract in one paragraph: files, behavior delta, the test
     that proves it.
  3. If testable: write the failing test first.
  4. Implement directly. Read-only exploration may be delegated to
     subagents; code writes must not.
  5. Fast gate:
       - Backend changed → cargo fmt && cargo clippy -p <crate> --all-targets
         -- -D warnings && cargo test -p <crate>.
       - Frontend changed → pnpm --filter @epifly/ui exec svelte-check &&
         pnpm --filter web check-types && pnpm --filter native check-types &&
         pnpm --filter @epifly/features test.
       - Compose changed → docker compose config --quiet.
  6. Browser/simulator verification per the phase's Verify steps. Capture
     URL, status, console errors, network entries, SQL probe output. A
     finding == a fix, same phase.
  7. Tick the checklist line. Commit with body:
        <one-line subject>

        Phase <n>.<m> — <what changed>
        Verified: <what was actually exercised, where>
        Deferred: <anything pushed forward, with reason>
  8. Per-phase gate (mandatory before next phase):
       - All checklist items for this phase ticked.
       - Web + iOS verification both green (skip a track only when the
         phase explicitly says N/A).
       - pnpm test:e2e:web for any web-touching phase.
       - Reviewer checklist re-checked for the slice this phase changed.
  9. Scope guard: if a step needs changes outside its declared phase scope,
     STOP. Write an "unplanned scope" note in the commit body and discuss
     before continuing.
end loop until Phase 9 is complete and the reviewer checklist is fully ticked.
```

### Stop / pause triggers

- Zitadel unreachable or misconfigured → pause until Z.1–Z.9 fixed.
- Any verification step fails twice with the same root cause → escalate.
- Step needs to touch three+ unrelated files → split.
- Any token-handling code introduced without a test → block.
- Any new log call within `apps/backend/crates/agent-gateway/src/{routes,mw}/auth*` without an explicit redaction review → block.

---

## Out of scope (explicitly deferred)

- External IdPs (Google/Apple/GitHub) configured **inside** Zitadel — Phase 3
  reads the list from a backend endpoint; the Zitadel-side wiring is
  operator work.
- Bespoke `@simplewebauthn` flow outside Zitadel hosted — Zitadel exposes the
  passkey factor; standalone WebAuthn is a later plan.
- Multi-region Zitadel HA.
- Audit log export to SIEM.
- Account deletion / GDPR export flow.
- Domain-verified enterprise onboarding (`email` domain → `org` auto-bind).
- `tauri-plugin-stronghold` for a broader encrypted vault.
- **DPoP / sender-constrained access tokens** for BFF→gateway and native→gateway. Reduces bearer-token replay risk; modern OAuth direction per RFC 9700. Worth naming so it does not get smuggled into Phase 7.
