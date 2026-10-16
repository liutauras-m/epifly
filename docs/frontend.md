# Frontend Architecture & Implementation

_Last reviewed: 2026-10-16._

This document describes the **ConusAI frontend monorepo** — the three shared packages
(`@conusai/ui`, `@conusai/sdk`, `@conusai/types`) and the two consumer apps
(`apps/web`, `apps/browser-shell`). It is the canonical reference for how UI, state,
streaming, auth, theming, and platform glue fit together.

The companion documents are [docs/ui-design.md](ui-design.md) (visual language),
[docs/ui-plan.md](ui-plan.md) (phased migration plan), and [docs/arch.md](arch.md)
(backend / platform architecture).

---

## 1. Goals & shape

| Concern                | Decision                                                                       |
| ---------------------- | ------------------------------------------------------------------------------ |
| One UI, three runtimes | Browser (SvelteKit SSR), macOS/Linux/Windows desktop (Tauri 2), iOS + Android (Tauri 2 mobile). |
| Shared component layer | `packages/ui` — Svelte 5 + runes, design tokens, motion, stores, screens.       |
| Shared transport layer | `packages/sdk` — typed REST + SSE client; no Svelte deps.                       |
| Shared type layer      | `packages/types` — domain types + generated OpenAPI types.                      |
| Visual identity        | Single source of truth in [packages/ui/tokens/tokens.json](../packages/ui/tokens/tokens.json); regenerates CSS + TS. |
| Cross-app parity       | [scripts/check-cross-app-imports.mjs](../scripts/check-cross-app-imports.mjs) blocks any `apps/X → apps/Y` import and any feature `.svelte` outside `packages/ui`. |
| State + reactivity     | Svelte 5 runes (`$state`, `$effect`, `$derived`). No stores from `svelte/store`. |
| Test surface           | Vitest (unit), Playwright (web E2E + visual), WebdriverIO + Appium (desktop + iOS E2E). |

The rule is mechanical: a screen, feature, primitive, store, motion primitive, theme
adapter, or capability renderer **must live in `packages/ui`**. The apps own only
routing, environment-specific bootstrapping, and a thin `sdk.ts` wrapper.

---

## 2. Repository layout

```
packages/
  types/                 # @conusai/types — domain types + OpenAPI types
  sdk/                   # @conusai/sdk   — REST/SSE client (Svelte-free)
  ui/                    # @conusai/ui    — tokens, primitives, features, screens
apps/
  web/                   # SvelteKit SSR app (adapter-node, port 5173)
  browser-shell/         # SvelteKit + Tauri 2 shell (port 5174 + Rust)
scripts/
  build-tokens.mjs           (lives under packages/ui/scripts; see §4)
  check-cross-app-imports.mjs
  check-design-tokens.mjs
  cross-platform-diff.mjs
  openapi-to-types.sh
turbo.json               # build graph (apps depend on packages)
```

The pnpm workspace declares all three packages as `workspace:*`, so every
`@conusai/*` import resolves through Vite's resolver (see
[apps/web/vite.config.ts](../apps/web/vite.config.ts) `ssr.noExternal`).

---

## 3. `packages/types`

Tiny package, zero runtime code. Two files:

- [src/domain.ts](../packages/types/src/domain.ts) — hand-written domain types
  that mirror Rust structs in `apps/backend/crates/common`. Examples:
  `WorkspaceNode`, `CapabilityCard`, `SessionTrace`, `UserStep`, `FileToken`.
- `src/openapi.d.ts` — **generated** from `apps/backend`'s OpenAPI spec by
  [scripts/openapi-to-types.sh](../scripts/openapi-to-types.sh) (`pnpm openapi:gen`).
  Not committed; the package's `prebuild` script regenerates it best-effort.

`index.ts` re-exports `./domain.js`. The OpenAPI types are opt-in (deep imports)
so non-backend consumers don't depend on generation.

Parity is enforced at build time by `scripts/assert-parity.js` (`pnpm types:assert-parity`).

---

## 4. `packages/ui`

The shared UI surface. Roughly 60 components, 7 screens, 10 stores, a motion
system, a capability renderer registry, an i18n helper, a theme system, and a
deterministic design-token pipeline.

### 4.1 Folder map (`packages/ui/src/lib`)

| Folder           | Purpose                                                                              |
| ---------------- | ------------------------------------------------------------------------------------ |
| `assets/`        | Logos, fonts (Geist variable + Geist Mono), favicon, lottie. Consumed via `?url`.    |
| `components/`    | Visual primitives + shell chrome (`Button`, `Field`, `Composer`, `Sidebar`, `Sheet`, `AppHeader`, `MessageBubble`, `ToolCard`, `DataTable`, …). Every component ships a `.fixtures.ts` sibling for the gallery. |
| `features/`      | Composite domain components (`AgentChatStream`, `WorkspaceTree`, `CapabilityBrowser`, `ProfileSheet`, `AttachmentSheet`) and the `createChatStream` factory. |
| `features/screens/` | Top-level views: `ChatScreen`, `CapabilitiesScreen`, `ArtifactsScreen`, `CapabilityDetailSheet`. |
| `features/workspace/` | Cross-screen dialogs (`ConfirmDialog`, `MoveDialog`, `NewNodeDialog`, `ShareDialog`). |
| `features/billing/` | `InvoiceStatusBadge` plus billing widgets (`PlanBadge`, `PlanCard`, `UsageMeter`, `QuotaBanner`, `QuotaList`). |
| `stores/`        | Runes-based reactive stores: `themeStore`, `toast`, `drawerStore`, `breadcrumbsStore`, `recentsStore`, `screenStore`, `modeStore`, `featureFlags`, `pins`. |
| `motion/`        | `springAnimate`, FLIP helpers (`recordRect` / `playFlip`), `stagger`, `tap`, `startViewTransition`, plus `keyframes.css` and a typed `SpringOpts`. |
| `live/`          | `createLiveResource.svelte.ts` — SWR + invalidation primitive (see §4.6).            |
| `routing/`       | `initialRoute` + `applyInitialRoute` — router-agnostic deep-link restore.            |
| `capabilities/`  | `CapabilityRendererRegistry` (plain + `.svelte.ts` context provider) so backend capabilities can plug custom rich renderers into chat. |
| `utils/`         | `actions.ts` (e.g. `autoGrow`), `haptics`, `i18n`, `keyboard`, `md`, `motion-prefs`, `platform`, `LiveAnnouncer.svelte`. |
| `tokens.css`     | **Generated.** Hand-editing is forbidden (Phase 2.1a).                               |
| `foundry.css`    | Foundation reset + base typography. Imported into a low-priority `@layer foundry`.   |

Every public symbol is re-exported from [src/lib/index.ts](../packages/ui/src/lib/index.ts);
deeper consumers can also import from the subpath maps declared in `package.json`
(`@conusai/ui/components/*`, `@conusai/ui/features`, `@conusai/ui/stores`, etc.).

### 4.2 Design token pipeline

Single source of truth: [packages/ui/tokens/tokens.json](../packages/ui/tokens/tokens.json).
Compiler: [scripts/build-tokens.mjs](../packages/ui/scripts/build-tokens.mjs)
(`pnpm --filter @conusai/ui tokens:build`, runs as the package's `build` step).

It emits two files:

- `src/lib/tokens.css` — CSS custom properties grouped into themed blocks
  (`:root[data-theme="paper"]`, `:root[data-theme="forge"]`) plus theme-invariant
  brand scalars, then **semantic aliases** (`--color-bg`, `--color-fg-muted`,
  `--color-accent`, etc.). Component CSS uses only the semantic aliases — never raw
  scalars — so re-skinning is a single JSON edit.
- `tokens/tokens.d.ts` — `FoundryToken` union of every token name, for type-safe
  references in TS.

The `apps/web/src/app.css` then bridges Foundry tokens into shadcn-svelte / Tailwind
v4 via `@theme inline { --color-primary: var(--ember); … }`, keeping the entire app
on one palette without duplication.

Guards in CI:

- `scripts/check-design-tokens.mjs` — every `.svelte` file uses only declared tokens.
- `packages/ui/scripts/check-motion-durations.mjs` and `check-motion-purpose.mjs` —
  motion budget (no rogue transitions, every animation has a `data-motion-purpose`).
- `packages/ui/scripts/check-no-local-components.mjs` — apps may not redeclare a
  primitive that already exists in `packages/ui`.
- `packages/ui/scripts/check-ui-contracts.mjs` — fixture / contract assertions per
  component.

### 4.3 Theming

Themes are switched by toggling `document.documentElement.dataset.theme`
(`"paper"` ↔ `"forge"`). The flow:

1. [components/ThemeProvider.svelte](../packages/ui/src/lib/components/ThemeProvider.svelte)
   creates a [stores/themeStore.svelte.ts](../packages/ui/src/lib/stores/themeStore.svelte.ts)
   instance (optionally injected with a `ThemeAdapter` for SSR persistence) and
   publishes it via `setContext('conusai.theme', …)`.
2. `ThemeScript.ts` exports an inline `THEME_SCRIPT` string that the apps embed in
   `app.html` to set the `data-theme` attribute **before first paint** — eliminating
   the dark-mode flash.
3. `ThemeSwitcher.svelte` flips the store; `ThemeProvider`'s `onThemeChange`
   callback lets the browser-shell forward the change to Rust via `emit('theme-change', …)`
   so native chrome can follow.

### 4.4 Reactivity model

The package is **runes-only**. Stores are plain classes that hold `$state` fields
and return getters; consumers read `store.x` directly, no `$store` syntax. Examples:

- `themeStore.current` / `.preference` / `.toggle()`.
- `toasts.push(t)`, `toasts.list` (the `ToastHost` component renders them).
- `drawerStore.open` / `.close()`, `breadcrumbsStore.set(node)`, `recentsStore.add(id)`.

A known caveat is documented inline in
[features/createChatStream.svelte.ts](../packages/ui/src/lib/features/createChatStream.svelte.ts):
mutations to a `$state` `Map` do not propagate to descendant `{#each}` templates, so
the factory bumps a sibling `toolCardsVersion` counter and also publishes a flattened
`toolCardsList` array. Don't remove either without verifying iOS Playwright still
sees tool cards appear.

### 4.5 Chat streaming primitive

[`createChatStream(sdk, opts)`](../packages/ui/src/lib/features/createChatStream.svelte.ts)
is the heart of the chat UX. It returns a controller exposing:

- `messages` (`ChatMessage[]`) — user / `ai` / `thinking` turns with per-word
  hydration metadata (`words: { t, id, delay }[]`) used by `MessageBubble` for the
  staggered fade-in animation.
- `toolCards` (`Map<id, ToolCardEntry>`) plus `toolCardsList` + `toolCardsVersion`.
- `inFlight`, `activeThreadId`, `lastRoutingMeta`, `lastInvalidation`, `lastSend`.
- `send(prompt, opts)`, `abort()`, `newSession()`, `loadThread(id)`.

It accepts an optional `streamFn` override — the browser-shell passes
[streamChatTauri](../apps/browser-shell/src/lib/tauri-stream.ts) so that, inside
WKWebView (which buffers SSE responses entirely before exposing them to JS), the
HTTP request is performed in Rust and chunks are forwarded as Tauri events
(`chat:chunk:<streamId>`). The web app uses the default `sdk.chat.stream(…)`
path, which is plain SSE over `fetch`.

The default streamer ([packages/sdk/src/chat.ts](../packages/sdk/src/chat.ts))
implements:

- POST `/ui/stream` with `{ message, thread_id?, workspace_node_id?, attachment_ids?, forced_capability? }`.
- Server-Sent Events parser (`data: … \n\n`) with `[DONE]` termination.
- Deltas: `text`, `tool_start`, `tool_result` (extracts `error: …` prefix),
  `routing_meta`, `resource_invalidated`, `thread_id`, `done`.
- Auto-reconnect with `[200, 600, 1800] ms` backoff, disable via
  `{ reconnect: false }`.

`createChatStream` additionally:

- Flushes accumulated words into `messages[aiIdx].words` on `requestAnimationFrame`
  for jitter-free streaming.
- Tracks per-stream inactivity (45 s) and aborts via the shared `AbortController`.
- Extracts `public_url`, `metadata.root_path`, `metadata.framework` from
  `host_project` tool results to populate the hosted-project surfaces.
- Forwards `resource_invalidated` deltas to any `createLiveResource`s that
  registered via `liveResource.subscribeToStream(chatStream)`.

### 4.6 Live resources (SWR)

[`createLiveResource(name, fetcher, { tenantId })`](../packages/ui/src/lib/live/createLiveResource.svelte.ts)
gives any data fetch the following lifecycle:

- Initial fetch on mount.
- Background refetch when the tab becomes visible after `IDLE_THRESHOLD_MS` (60 s).
- Invalidation on matching SSE `resource_invalidated` deltas (scope-filtered when
  `tenantId` is supplied — PR 3.A.7's defensive cross-tenant guard).
- Exponential backoff `[500, 2000, 8000] ms` on fetch failure.
- `mutate(updater, { rollbackOn: Promise })` for optimistic updates with structural
  clone (no immer — incompatible with Svelte 5 reactive proxies); auto-rollback +
  toast on rejection.

### 4.7 Capability renderer registry

The chat surface lets backend capabilities ship bespoke rich renderers. The flow:

- `provideCapabilityRendererRegistry()` is called once in the app's root page; it
  creates a registry and publishes it via Svelte context (see
  [capabilities/CapabilityRendererRegistry.svelte.ts](../packages/ui/src/lib/capabilities/CapabilityRendererRegistry.svelte.ts)).
- Components inside `AgentChatStream` / `ToolCallCard` call
  `useCapabilityRendererRegistry()` and look up a renderer by `card.name`.
- Apps (or capabilities themselves) call `registry.register(name, Component)` —
  enabling a plugin model without a central switch statement.

### 4.8 Shell composition

The single mount point for both apps is
[`<ShellPage>`](../packages/ui/src/lib/features/ShellPage.svelte) — it consumes
`sdk`, `chatStream`, `userName`, `userPlan`, `sigil`, `appTitle`, plus
router-agnostic `onLogout` / `onWorkspaceChange` / `onUnknownRoute` callbacks. It
runs `initialRoute()` + `applyInitialRoute()` on mount to restore deep links
(`?ws=<id>`, `?screen=…`), wires the current selection to the breadcrumbs and
recents stores, and renders `<ShellScreen>` (which lays out `AppHeader` + `Sidebar`
+ active screen). Each app supplies its own router glue.

For unauthenticated state both apps render the same
[`<ShellLoginScreen>`](../packages/ui/src/lib/features/ShellLoginScreen.svelte).

---

## 5. `packages/sdk`

A small, dependency-light TypeScript client. No Svelte. No fetch polyfills.
Everything is plain async functions and async generators.

### 5.1 Construction

```ts
import { createConusSdk } from '@conusai/sdk';

const sdk = createConusSdk({
  fetch,                         // injected (browser fetch, server fetch, Tauri fetch wrapper)
  baseUrl: '',                   // both apps pass '' and let their fetch wrapper pick the origin
  tokenProvider: { get: async () => bearerOrNull },
});
```

A non-null token from `tokenProvider` is sent as `Authorization: Bearer <token>`.
Neither app uses it: the web app's browser code never holds a token (the
SvelteKit server adds it, §6.3) and the shell's fetch wrapper adds the bearer
itself (§7.2).

[`client.ts`](../packages/sdk/src/client.ts) builds an `InternalClient` exposing
`request<T>` (throws on non-2xx) and `call<T>` (returns `ApiResult<T>` —
`{ data, error: { status, message } }`). All resource modules consume this
internal client.

### 5.2 Resource modules

| Module          | Surface                                                                          |
| --------------- | --------------------------------------------------------------------------------- |
| `capabilities.ts` | List, search, register / unregister capability cards. `RegisterCapabilityRequest`. |
| `chatApi.ts`    | `sdk.chat.stream({...})` wrapping `streamChat` from `chat.ts`.                    |
| `files.ts`      | Upload, presigned URLs, extract-invoice.                                          |
| `threads.ts`    | `sdk.threads.list`, `.get`, `.messages` — pagination + delete.                    |
| `ui.ts`         | UI-facing endpoints (`/ui/upload`, `/ui/extract-invoice`).                        |
| `workspaces.ts` | Tree, search, content, share/unshare, move.                                       |
| `glyphs.ts`     | `glyphFor(name)` — capability → icon name mapping.                                |

`endpoints.ts` is the **only** place URL paths live (`EP.UI_STREAM`,
`EP.WORKSPACES_TREE`, …). Refactor a path here and every caller follows.

`types.ts` contains the SDK's own narrow types (`ApiError`, `ApiResult`,
`ChatStreamDelta`, `RoutingMeta`, `UploadResponse`, `InvoiceData`,
`WorkspaceContent`). Domain types come from `@conusai/types`.

### 5.3 Streaming details

See §4.5. The SDK never depends on a global `fetch` — apps inject one. This is
critical for SvelteKit SSR (server loads use `gatewayFetch`, §6.4) and for the
Tauri shell (which wraps fetch to resolve relative URLs against
`VITE_GATEWAY_URL` and add `Authorization: Bearer`, §7.2).

---

## 6. `apps/web` — SvelteKit (browser)

SvelteKit on `adapter-node` (port 3000 in the container, 5173 under `vite dev`),
acting as a **backend-for-frontend**: it signs users in against Zitadel on the
server, keeps the tokens in an encrypted cookie and proxies API calls to the
gateway (`GATEWAY_URL`) with the bearer token added. The browser only ever talks
to this origin. Renders the workshop in SSR for the first paint, hydrates with
runes.

### 6.1 Stack

`@sveltejs/kit 2.21`, `svelte 5.33`, `@tailwindcss/vite 4.3`, `bits-ui 2.18`,
`tailwind-variants`, `lucide-svelte`. Tests: `@playwright/test`,
`@axe-core/playwright`, `lighthouse`, `vitest`.

### 6.2 Routes (`src/routes/`)

| Path                         | Purpose                                                                 |
| ---------------------------- | ----------------------------------------------------------------------- |
| `+layout.svelte`             | Imports `app.css`, mounts `<ThemeProvider>` + `<LiveAnnouncer>` + `<ToastHost>`, bootstraps i18n, sets `data-hydrated="true"` for E2E waiters. |
| `+layout.server.ts`          | Returns display data only — `{ user: { name, email, plan, firstName, initials } }` (plan from `GET /v1/billing/subscription`), or `{ user: null }`. Never a token. |
| `+page.svelte`               | The workshop. Provides capability registry, instantiates `createChatStream`, mounts `<ShellPage>` with SvelteKit `goto` as the router; logout is a hidden `POST /auth/logout` form. |
| `+error.svelte`              | Error boundary using `<EmptyState kind="error">`.                       |
| `auth/login/+server.ts`      | Starts the Zitadel authorization-code flow (PKCE S256, `state`, `nonce`); `?returnTo=` picks the landing page. |
| `auth/callback/+server.ts`   | Verifies the sealed transaction, exchanges the code, creates the session cookie, redirects to `returnTo`. |
| `auth/logout/+server.ts`     | `POST` only: revokes the refresh token, clears the cookie, redirects to Zitadel's `end_session` endpoint. |
| `auth/signed-out/`           | Post-logout landing page ("Sign in again").                             |
| `healthz/+server.ts`         | Liveness probe (`ok`), no auth, no upstream calls.                       |
| `v1/`, `api/`, `ui/`, `mcp/`, `admin/` (`[...path]`) | Same-origin proxy to the gateway (§6.4).                       |
| `account/+page.svelte`       | Account overview.                                                       |
| `account/billing/`           | Subscription, checkout / customer-portal hand-off via the gateway's `/v1/billing/*`. |
| `account/usage/`             | Plan usage + quotas.                                                    |
| `_/ui/+page.svelte`          | **Dev-only primitive gallery** — renders every component with its fixtures (theme + viewport controls). Guarded in `_/ui/+layout.ts`. |

### 6.3 Session & auth

The server is an OIDC client of Zitadel; the browser never sees a token.

- **Configuration** — [src/lib/server/config.ts](../apps/web/src/lib/server/config.ts)
  is the whole environment contract: `ORIGIN`, `GATEWAY_URL`, `ZITADEL_ISSUER`,
  `ZITADEL_WEB_CLIENT_ID`, optional `ZITADEL_WEB_CLIENT_SECRET` (confidential
  client only) and `AUTH_SESSION_KEY` (32 random bytes, base64). The `init`
  hook validates it and the server refuses to start when anything is missing
  or malformed. `BODY_SIZE_LIMIT` is read by adapter-node. Values per
  deployment: [deploy-conusai-cloud.md](deploy-conusai-cloud.md).
- **Login** — [src/lib/server/auth/oidc.ts](../apps/web/src/lib/server/auth/oidc.ts)
  runs discovery against `ZITADEL_ISSUER` (`openid-client`) and requests
  `openid profile email offline_access`. `/auth/login` seals `state`, `nonce`,
  the PKCE verifier and `returnTo` into the 10-minute cookie `__Host-epifly_oidc`
  ([transaction.ts](../apps/web/src/lib/server/auth/transaction.ts));
  `/auth/callback` checks them and exchanges the code. The redirect URI is
  always `${ORIGIN}/auth/callback`, the post-logout URI `${ORIGIN}/auth/signed-out`.
- **Session** — [src/lib/server/auth/session.ts](../apps/web/src/lib/server/auth/session.ts)
  seals `{ user, access token, expiry, refresh token }` with AES-256-GCM
  (`AUTH_SESSION_KEY`, [seal.ts](../apps/web/src/lib/server/auth/seal.ts)) into
  the cookie `__Host-epifly_session` (Secure, HttpOnly, SameSite=Lax, Path=/),
  split over `.1`/`.2` when larger than 3800 bytes. There is no server-side
  store; the session lives at most 30 days. The ID token is not kept.
- **Refresh** — an access token with less than a minute left is refreshed with
  the refresh token. Refreshes are single-flight per refresh token, and a
  finished refresh is remembered for 30 s so parallel requests carrying the old
  cookie reuse the rotated tokens. A dead grant (`invalid_grant`) clears the
  session.

[src/hooks.server.ts](../apps/web/src/hooks.server.ts) per request:

1. Sets security headers (`nosniff`, `X-Frame-Options: DENY`, referrer and
   permissions policy, HSTS on https).
2. Resolves the session into `event.locals.session`. Pages without a session
   redirect to `/auth/login?returnTo=…`; `/healthz`, `/auth/*` and (dev only)
   `/_/ui` are public; proxied paths answer `401` themselves.
3. `transformPageChunk` — finds the content-hashed Geist + Geist Mono `.woff2`
   URLs Vite injected and prepends matching `<link rel="preload">` to eliminate
   font swap.

`svelte.config.js` keeps SvelteKit's origin check on for form posts and
configures a **nonce-based CSP** with `connect-src 'self'` only — every API
call is same-origin.

### 6.4 SDK wiring and the gateway proxy

[src/lib/sdk.ts](../apps/web/src/lib/sdk.ts) — the browser SDK uses `baseUrl: ''`
and no token: every call goes to the app's own origin.
[src/lib/server/proxy.ts](../apps/web/src/lib/server/proxy.ts) forwards `/v1`,
`/api`, `/ui`, `/mcp` and `/admin` to `GATEWAY_URL`:

- injects `Authorization: Bearer <access token>` from the session and drops
  inbound `Authorization`, `Cookie`, `X-Tenant-ID`, `X-Session-Token`,
  `X-Device-Token`, `X-API-Key`, forwarding and hop-by-hop headers; passes
  `x-request-id` (generated when absent), `traceparent`, `tracestate`;
- refuses non-safe methods that are not same-origin;
- streams bodies (cap 100 MB), streams SSE responses, strips `Set-Cookie`;
- does **not** proxy WebSocket upgrades (adapter-node has no upgrade hook).

Server-side loads and form actions call the gateway directly through
[src/lib/server/gateway.ts](../apps/web/src/lib/server/gateway.ts)
(`gatewayFetch(session)`, `serverSdk(session)`, `requireSession(locals)`).

### 6.5 Vite config

- `tailwindcss()` plugin first, then `sveltekit()`.
- `ssr.noExternal: ['@conusai/sdk', '@conusai/ui', '@conusai/types']` so the
  workspace packages go through Vite's resolver (Node's ESM loader can't handle
  the `.js` extensions in TS re-exports under @tailwindcss/node).
- `server.fs.allow: ['../../packages/ui']` — required for hot module reloading
  cross-package.
- No dev proxy: under `vite dev` the SvelteKit server itself proxies to
  `GATEWAY_URL`, exactly as in production, so `just web-dev` needs the full
  `config.ts` environment; `ORIGIN` must be the dev URL and its
  `/auth/callback` a redirect URI registered on the Zitadel web app.

### 6.6 Local UI surface in the app

`src/lib/components/ui/` holds the **shadcn-svelte** wrappers (`alert`,
`alert-dialog`, `avatar`, `badge`, `breadcrumb`, `button`, `card`,
`dropdown-menu`, `input`, `item`, `label`, `progress`, `radio-group`,
`separator`, `sheet`, `sidebar`, `skeleton`, `tooltip`). These are intentionally
allowed in `apps/web` because they are thin bindings around `bits-ui` — but
**no new feature components** may land here; the cross-app lint enforces it.

The runtime hook `src/lib/hooks/is-mobile.svelte.ts` is the only non-UI hook.

### 6.7 Tests

`apps/web/e2e/`:

- `smoke.test.ts` — `/healthz`, redirect of an unauthenticated `/` to `/auth/login`, signed-out page and its nonce CSP.
- `keyboard.spec.ts` — focus traps, Cmd-K, slash command.
- `motion-budget.spec.ts` — asserts every animation has `data-motion-purpose`.
- `visual/visual.spec.ts`, `visual/reduced-motion.spec.ts` — Playwright + axe-core
  visual regression and accessibility.

---

## 7. `apps/browser-shell` — SvelteKit + Tauri 2 (desktop + mobile)

Same Svelte 5 source, different runtime: a SvelteKit static build (port 5174)
loaded into a Tauri WebView. Targets `app`, `dmg`, `msi`, `appimage`, `deb`,
iOS 16+, Android `minSdkVersion 26`.

### 7.1 Stack

- Frontend: SvelteKit (`adapter-static`), Svelte 5.33, `@tauri-apps/api 2`,
  `@tauri-apps/plugin-dialog 2`, `vite-plugin-static-copy` (mirrors `packages/ui/src/lib/assets` into the
  bundle so logos / fonts ship offline).
- Rust: Tauri 2 (`unstable` features), `tauri-plugin-haptics`, `tauri-plugin-dialog`,
  `tauri-plugin-stronghold` + `iota_stronghold` (used from Rust only, argon2 KDF —
  its JS commands are not registered),
  `tauri-plugin-webdriver-automation` (debug + macOS + `e2e` feature only),
  `reqwest`, `tokio`, `futures-util`, `ulid`, `sha2`, `rand`, `open`.
- Workspace crate dependency: `common` from `apps/backend/crates/common`.

### 7.2 Frontend layout (`src/`)

- `routes/+layout.svelte` — same role as the web app: foundry CSS, theme
  provider, i18n, hydrated marker; additionally emits `theme-change` events to
  Rust on theme switch.
- `routes/+page.svelte` — restores the session from the Rust vault on mount;
  renders `<ShellLoginScreen>` (sign-in button) or `<ShellPage>`. Passes a Tauri-specific `streamFn` (see §4.5) and uses
  `replaceState` from `$app/navigation` for workspace ↔ URL sync.
- `routes/+layout.ts` — disables SSR (`export const ssr = false; export const prerender = false`).
- `lib/sdk.ts` — Tauri-aware SDK. The `fetch` wrapper resolves bare paths
  against the build-time `VITE_GATEWAY_URL`, asks Rust for an access token on
  every request (`invoke('auth_access_token')`; Rust refreshes it when needed)
  and sends `Authorization: Bearer`, with `credentials: 'omit'`. A `401` marks
  the session expired. No token is stored in the webview.
- `lib/session.svelte.ts` — mirrors who is signed in (`auth_status`,
  `auth_sign_in`, `auth_sign_out`); holds no token.
- `lib/tauri-stream.ts` — alternative streamer that proxies SSE through Rust
  (the only correct workaround for WKWebView SSE buffering, confirmed in
  `tauri-apps/plugins-workspace#2415`, `#2129`).
- `lib/mobile/platform/detect.ts` — sets `data-platform` on `<html>` for
  iOS/Android/desktop conditional CSS.

### 7.3 Rust side (`src-tauri/src/`)

| File              | Responsibility                                                                                             |
| ----------------- | ---------------------------------------------------------------------------------------------------------- |
| `main.rs`         | Thin entrypoint → `lib::run()`.                                                                            |
| `lib.rs`          | Builds the Tauri app: registers plugins (`dialog`, `haptics`; no Stronghold or HTTP plugin), manages four state handles (tabs, recorder, auth, stream registry), registers all `#[tauri::command]`s, and (in debug + macOS + `e2e` feature) the WebDriver automation plugin. Also exports a `RECORDER_BRIDGE_JS` injected into child tab webviews to forward DOM events to the recorder. |
| `chat_stream.rs`  | `chat_stream_start` / `chat_stream_abort` commands. POSTs to `/ui/stream`, parses SSE in Rust, emits `chat:chunk:<streamId>` events with strongly-typed `ChunkPayload` enum. Bypasses WKWebView SSE buffering. |
| `auth/mod.rs`     | `auth_status` / `auth_sign_in` / `auth_sign_out` / `auth_access_token` commands (main window only). Sign-in: binds `127.0.0.1:<random port>`, opens the system browser (`open` crate) on Zitadel's authorize endpoint (code + PKCE S256, `state`, `nonce`), waits for `/callback`, exchanges the code, validates the ID token. Keeps the access token in memory and serialises refreshes behind one mutex. Sign-out revokes the refresh token and opens `end_session`. Build-time `VITE_ZITADEL_ISSUER`, `VITE_ZITADEL_NATIVE_CLIENT_ID`, `VITE_GATEWAY_URL` (`option_env!`). |
| `auth/oidc.rs`    | Discovery, authorization request, loopback callback listener, code exchange, refresh, revocation, ID-token validation. |
| `auth/vault.rs`   | Refresh-token vault: Stronghold snapshot keyed with argon2 over a per-install random salt (`vault.salt`). Only Rust reads it. |
| `registration.rs` | `upload_trace_cmd` uploads a session trace to the workspace with the bearer token (main window only). |
| `tabs.rs`         | Browser-tab manager (`create_tab`, `close_tab`, `navigate_tab`, `list_tabs`, `save_tabs`, `restore_tabs`). |
| `recorder.rs`     | `recorder_start` / `recorder_record_step` / `recorder_stop` / `recorder_status`. Receives DOM events from the injected bridge JS, redacts PII (`password|ssn|cc-|card|cvv`), saves traces. |
| `telemetry.rs`    | `tracing` subscriber + log routing.                                                                        |

### 7.4 Configuration

[tauri.conf.json](../apps/browser-shell/src-tauri/tauri.conf.json):

- `frontendDist: "../build"` (the SvelteKit static output).
- Dev URL `http://localhost:5174`; `beforeDevCommand` runs `pnpm --filter browser-shell dev`.
- The CSP in `tauri.conf.json` allows no remote origin. Run the CLI through
  `pnpm --filter browser-shell tauri …` ([scripts/tauri.mjs](../apps/browser-shell/scripts/tauri.mjs)):
  for `dev`/`build` it requires `VITE_GATEWAY_URL`, `VITE_ZITADEL_ISSUER` and
  `VITE_ZITADEL_NATIVE_CLIENT_ID` and pins `connect-src`/`img-src` to the
  gateway origin. The issuer is not in `connect-src`: only Rust talks to Zitadel.
- `deep-link` config: mobile host `open`, desktop scheme `conusai:` (shareable
  workspace URLs; sign-in uses the loopback redirect, not a deep link).
- Sign-in works on desktop only: mobile (iOS/Android) sign-in is not yet
  implemented.
- Capabilities split between `capabilities/main-capability.json` (full desktop
  privileges) and `capabilities/ios-capability.json` (the minimal mobile set).

### 7.5 E2E

- `e2e/` (under `src-tauri/`) contains the WebDriver-driven specs that run
  against the macOS build when the `e2e` Cargo feature is enabled.
- `e2e/wdio/` at the repo root drives the desktop WebView via WebdriverIO.
- `e2e/ios/` drives the iOS simulator via Appium + the same `__conusaiSdk`
  window-attached SDK (guarded by `VITE_E2E_EXPOSE_SDK`).

---

## 8. Cross-cutting workflows

### 8.1 Build graph (`turbo.json`)

```
build: { dependsOn: ["^build"], outputs: [".svelte-kit/**", "dist/**", "build/**"] }
dev:   { cache: false, persistent: true }
lint:  { dependsOn: ["^build"] }
test:  { dependsOn: ["^build"] }
check-types: { dependsOn: ["^build"] }
```

The `^build` topological dep means `pnpm build` always builds packages before
apps. Tokens build is part of `@conusai/ui`'s `build`, so apps always see fresh
`tokens.css` / `tokens.d.ts`.

### 8.2 Lint / format

- `biome.json` — formatter + linter for everything except Svelte.
- `eslint.config.js` — Svelte parsing + a11y rules + Svelte 5 runes plugin.
- `scripts/check-cross-app-imports.mjs` — enforces "no app→app imports" and
  "no feature `.svelte` outside `packages/ui`" (with a tight allow-list for
  `+page.svelte`, `+layout.svelte`, `+error.svelte`, `MobileShell.svelte`,
  `MobileTopBar.svelte`).

### 8.3 Type generation

```
scripts/openapi-to-types.sh  # backend openapi.json → packages/types/src/openapi.d.ts
```

Run via `pnpm --filter @conusai/types openapi:gen`. The web app reads these
generated types when constructing server SDKs (e.g. `WorkspaceNode`).

### 8.4 Theming + dark mode without flash

1. `app.html` includes the inline `THEME_SCRIPT` (exported from
   `@conusai/ui`) to apply `data-theme` before first paint.
2. Server returns CSS with both themes (rules scoped by `:root[data-theme=…]`).
3. `<ThemeProvider>` hydrates and switching becomes purely client-side.

### 8.5 Request flow (`POST /ui/stream`, browser)

```
Composer (packages/ui)
  ↓ onSubmit
ShellScreen → ChatScreen → createChatStream.send(prompt)
  ↓ sdk.chat.stream({...})
@conusai/sdk → chatApi → streamChat
  ↓ POST /ui/stream  (same origin, apps/web/src/lib/sdk.ts)
SvelteKit proxy (adds Authorization: Bearer from the session) → Rust gateway (apps/backend)
  ↑ SSE: data: {choices:[{delta:{content|tool_call_start|tool_call_result|routing_meta|resource_invalidated}}], thread_id}
streamChat yields ChatStreamDelta
  → createChatStream pushes to $state messages / toolCards
  → MessageList / ToolCard re-render
  → createLiveResource consumers see resource_invalidated and refetch
```

In the Tauri shell the same path runs, except `createChatStream` is given the
`streamChatTauri` override; the HTTP request is performed in Rust
(`chat_stream_start` command) and the deltas arrive via Tauri events.

### 8.6 Sharing UI between web and Tauri

The exact same `<ShellPage>` mounts in both `apps/web/src/routes/+page.svelte`
and `apps/browser-shell/src/routes/+page.svelte`. Differences are limited to:

- Router glue (SvelteKit `goto` vs `replaceState`).
- SDK construction (same-origin fetch through the SvelteKit proxy vs direct gateway fetch with a bearer from Rust).
- Stream function override (default SSE vs Tauri-event SSE).
- Auth (server-side OIDC + encrypted cookie vs Rust loopback PKCE + Stronghold vault).
- Asset pipeline (Vite serves directly vs `vite-plugin-static-copy` mirrors
  `packages/ui/.../assets` into the bundle).

Everything else — every primitive, every store, every screen, every motion
helper, every i18n string — flows through `packages/ui`.

---

## 9. Where to add things

| Need to add…                                | Goes in…                                                                |
| ------------------------------------------- | ----------------------------------------------------------------------- |
| A new visual primitive                       | `packages/ui/src/lib/components/` + `.fixtures.ts` + add to `_/ui` gallery. |
| A new top-level screen                       | `packages/ui/src/lib/features/screens/` + export from `features/index.ts`. |
| A new domain feature component               | `packages/ui/src/lib/features/`. Never in `apps/*`.                     |
| A new backend endpoint                       | Add path to `packages/sdk/src/endpoints.ts`, add method to the appropriate resource module. |
| A new domain type                            | `packages/types/src/domain.ts` (and Rust mirror).                       |
| A new design token                           | Edit `packages/ui/tokens/tokens.json`, run `pnpm --filter @conusai/ui tokens:build`. |
| A new theme                                  | Add a `:root[data-theme="<name>"]` block in `tokens.json`.              |
| A new motion primitive                       | `packages/ui/src/lib/motion/`, then export from `motion/index.ts`.      |
| A new capability renderer                    | Register at runtime via `registry.register(name, Component)` — the registry lives in `packages/ui/src/lib/capabilities/`. |
| A new desktop/mobile-only native command     | `apps/browser-shell/src-tauri/src/<module>.rs` + `invoke_handler!` entry, then a thin TS wrapper in `apps/browser-shell/src/lib/`. |
| A new SvelteKit route (web only)             | `apps/web/src/routes/...` — but the page body should still compose `packages/ui` parts. |

---

## 10. Open issues / watch-outs

- **Map reactivity gap** in `createChatStream` (see §4.4) — keep both
  `toolCardsVersion` and `toolCardsList` until upstream Svelte fixes the
  proxy-trap on `Map.set`. Documented under §17 in `docs/verify/verify-ios.md`.
- **WKWebView SSE buffering** — always go through `streamChatTauri` in the
  Tauri shell; never fall back to the default SDK streamer on iOS / macOS.
- **No cookies in the shell** — the shell calls the gateway cross-origin with
  `Authorization: Bearer` and `credentials: 'omit'`; the gateway's CORS allows
  `tauri://localhost` and `http://tauri.localhost` in addition to `WEB_ORIGIN`.
- **CSP nonce vs HMR** — the web app uses nonce CSP in production; Vite dev
  injects its own scripts which need to remain `unsafe-inline`-tolerated only
  in dev.
- **Tokens are generated** — `packages/ui/src/lib/tokens.css` must never be
  hand-edited (Phase 2.1a). Always edit `tokens/tokens.json`.
- **No app→app imports** — the cross-app lint fails CI if any file under
  `apps/<a>/src` imports from `apps/<b>/src`. Share via `packages/ui`.
