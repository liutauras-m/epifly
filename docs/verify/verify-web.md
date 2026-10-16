# ConusAI Web (Desktop) — Verification Plan

Verification of the **`apps/web` SvelteKit app**. Companion to [`verify.md`](verify.md)
(gateway / data plane) and [`verify-ios.md`](verify-ios.md) (Tauri shell on the iOS simulator).

> **Architecture under test**: `apps/web` is a backend-for-frontend (SvelteKit +
> `@sveltejs/adapter-node`, image `apps/web/Dockerfile`, port 3000). It signs users in
> server-side against Zitadel (authorization code + PKCE + state + nonce), keeps the
> tokens in the AES-256-GCM encrypted cookie `__Host-epifly_session`, and proxies `/v1`,
> `/api`, `/ui`, `/mcp` and `/admin` to `GATEWAY_URL`, injecting
> `Authorization: Bearer <access token>` and dropping any inbound `Authorization`,
> `Cookie`, `X-Tenant-ID`, `X-Session-Token` or API-key header. The browser never holds a
> token. Server code: `apps/web/src/lib/server/`. Environment contract:
> `apps/web/src/lib/server/config.ts` (`ORIGIN`, `GATEWAY_URL`, `ZITADEL_ISSUER`,
> `ZITADEL_WEB_CLIENT_ID`, optional `ZITADEL_WEB_CLIENT_SECRET`, `AUTH_SESSION_KEY`); a
> missing or malformed value stops the server at start.
>
> The canonical driver is **Playwright**. `playwright.config.ts` starts two servers: a mock
> OIDC provider (`e2e/fixtures/mock-oidc.mjs`, port 4180) that approves every sign-in at
> once, and the built SSR server on port 4173 with a random `AUTH_SESSION_KEY` and a
> `GATEWAY_URL` where no gateway listens. The suite therefore covers the real sign-in flow
> and the UI with gateway calls failing soft or mocked per spec; it does not reach a gateway.

---

## Routes of the web server

| Route | Purpose |
|---|---|
| `GET /healthz` | Liveness, `200 ok`, no session needed |
| `GET /auth/login?returnTo=/path` | Starts the OIDC flow (redirect to Zitadel) |
| `GET /auth/callback` | Code exchange; sets the session cookie |
| `POST /auth/logout` | Revokes the refresh token, clears the cookie, redirects to Zitadel's end-session endpoint |
| `GET /auth/signed-out` | Post-logout landing page |
| `/v1/*`, `/api/*`, `/ui/*`, `/mcp/*`, `/admin/*` | Proxy to the gateway; `401` without a session; non-GET requests must be same-origin |
| any other page | Redirects to `/auth/login` without a session |

WebSocket upgrades are not proxied.

---

## Prerequisites

```bash
node --version                                   # v24 (the image uses node:24.22.0)
pnpm --version                                   # pnpm 10
pnpm install
pnpm exec playwright install --with-deps chromium webkit
```

---

## Phase 1 — Build the SSR bundle

```bash
pnpm --filter web build
ls apps/web/build/index.js                       # SSR entry
```

Playwright serves this build; rebuild after every frontend change.

---

## Phase 2 — Playwright `web` project (no backend needed)

```bash
pnpm e2e:web                                     # = playwright test --project=web
pnpm exec playwright test --project=web --headed --debug -g 'chat stream'
```

| Spec | Covers |
|---|---|
| `e2e/web/auth.spec.ts` | Redirect to `/auth/login`, sign-in through the mock IdP, the `__Host-epifly_session` cookie flags, forged cookie refused, cross-origin `POST` refused (`403`), sign-out |
| `e2e/web/greeting.spec.ts` | Greeting screen after sign-in |
| `e2e/web/chat-stream.spec.ts` | Composer and SSE chat rendering |
| `e2e/web/keyboard.spec.ts` | Keyboard parity (submit, newline, focus) |
| `e2e/web/live-resources.spec.ts` | Workspace tree and recents live refresh |
| `e2e/web/url-state.spec.ts` | URL state restoration |
| `e2e/web/tool-errors.spec.ts` | Tool error toast |
| `e2e/web/motion-budget.spec.ts` | Animation duration budget |

Specs sign in with `signIn()` from `e2e/helpers/auth.ts`, which runs the real
`/auth/login` → mock IdP → `/auth/callback` flow. Failing specs leave a trace and a
screenshot in `test-results/`; open a trace with `pnpm exec playwright show-trace <zip>`.

SSE assertions must wait for the stream to finish (the `.thinking` indicator is gone, and
the bubble holds the expected token or its text length is stable over two polls at least
750 ms apart); an early poll otherwise latches onto the first delta.

---

## Phase 3 — Live stack (`just up`)

1. Create `.env` and start the stack (Postgres, Redis, Qdrant and RustFS come from
   `docker-compose.local.yml`):

   ```bash
   scripts/generate-env.sh > .env && chmod 600 .env   # then set ANTHROPIC_API_KEY
   just up
   ```

2. Configure Zitadel at `http://localhost:8085` (admin user `ZITADEL_ADMIN_USERNAME` from
   `.env`) as described in [deploy-conusai-cloud.md](../deploy-conusai-cloud.md), with the
   web application's redirect URI `http://localhost:3000/auth/callback` and post-logout
   URI `http://localhost:3000/auth/signed-out`. Put the project id into `ZITADEL_AUDIENCE`
   and the web client id into `ZITADEL_WEB_CLIENT_ID`, then `just up` again. Until
   `ZITADEL_AUDIENCE` is set the gateway exits with code 2, and `web` waits for a healthy
   gateway.

3. Check:

   ```bash
   curl -s http://localhost:3000/healthz                                   # ok
   curl -s -o /dev/null -w '%{http_code}\n' http://localhost:3000/          # 303 → /auth/login
   curl -s -o /dev/null -w '%{http_code}\n' http://localhost:3000/v1/capabilities   # 401
   ```

4. In a browser open `http://localhost:3000`, sign in with a Zitadel user, send a chat
   message, attach a file, and sign out (`POST /auth/logout` lands on
   `/auth/signed-out`).

Known issue: in `docker-compose.local.yml` both the gateway and web use
`ZITADEL_ISSUER=http://localhost:8085`, which inside their containers is the container
itself, not Zitadel; OIDC discovery and JWKS fetches fail there until the issuer is
reachable under the same URL from the browser and from the containers.

Logs: `just logs web`, `just logs gateway`.

---

## Phase 4 — Dev server (hot reload)

```bash
just web-dev                                     # pnpm --filter web dev on :5173
```

There is no Vite proxy: in `vite dev` the SvelteKit server proxies `/v1`, `/api`, `/ui`,
`/mcp` and `/admin` to `GATEWAY_URL` exactly as in production. `just web-dev` sets only
`GATEWAY_URL`; the dev server also needs `ORIGIN=http://localhost:5173`,
`ZITADEL_ISSUER`, `ZITADEL_WEB_CLIENT_ID` and `AUTH_SESSION_KEY` in its environment, and
Zitadel needs `http://localhost:5173/auth/callback` registered as a redirect URI.

---

## Phase 5 — Live gateway use cases (UC1–UC5)

`e2e/ios/capabilities-business.spec.ts` holds the business use cases (invoice, contract,
medical claim, CV screening, incident report). They are skipped unless
`GATEWAY_INTEGRATION_TEST=1` is set, and the registry check additionally needs
`SUPER_TOKEN` (a Zitadel access token of a user with the `super_admin` role). The
Playwright web server is wired to the mock IdP and to no gateway, so these cases have no
working live configuration at present.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Web server exits at start with `[config] … is required` | Environment contract incomplete | Set the named variable (see `apps/web/src/lib/server/config.ts`) |
| `[config] AUTH_SESSION_KEY must be 32 bytes` | Key not base64 of 32 bytes | `openssl rand -base64 32` |
| Redirect loop or `invalid_request` at Zitadel | Redirect URI not registered | Register `<ORIGIN>/auth/callback` and `<ORIGIN>/auth/signed-out` on the Zitadel web app |
| Every proxied call returns `401` after sign-in | Gateway rejects the token: wrong `ZITADEL_AUDIENCE`, opaque access token, or no `urn:zitadel:iam:user:resourceowner:id` claim | Use the project id as audience and JWT access tokens. The organization claim needs the `urn:zitadel:iam:user:resourceowner` scope, which `LOGIN_SCOPE` in `apps/web/src/lib/server/auth/oidc.ts` does not request at present |
| `403` on a `POST` through the proxy | Request not same-origin | Call from the app's own origin |
| `500 Sign-in failed: no refresh token` at `/auth/callback` | Refresh-token grant not enabled | Enable the refresh-token grant on the Zitadel web app |
| Playwright started `:4173` when you wanted another server | `webServer` starts unless a server already answers | Stop the other process or let `reuseExistingServer` reuse it |

---

## Quick reference

```bash
pnpm --filter web build && pnpm e2e:web
```
