# Epifly — Claude Code instructions

## What this repo is

Svelte 5 + SvelteKit monorepo for the Epifly platform. Ships as a web app and a native app (iOS, Android, macOS, Windows) via Tauri v2. Backend is Rust (Axum). The frontend is not one app with platform detection hacks — it is two separate runtime apps sharing packages.

```
apps/web        SvelteKit web app — can use SSR and server routes
apps/native     SvelteKit SPA inside Tauri v2 — static only, no SSR
apps/backend    Rust/Axum API server

packages/sdk      Conus SDK — source of truth for all API access
packages/ui       shadcn-svelte primitives + shared product UI components
packages/features Svelte rune stores and feature actions (use SDK, never fetch directly)
packages/shared   Runtime-neutral constants, types, utilities
```

## Architecture rules

**Do not violate these. They are not preferences.**

1. Do not hardcode API paths outside `packages/sdk`. Use the `EP` endpoint map from `sdk/src/endpoints.ts`.
2. Do not parse SSE in components. Use `sdk.chat.stream` — it yields typed deltas.
3. Do not put product components inside `packages/ui/src/components/ui/`. That folder is for shadcn primitives only.
4. Do not use `export let` in new Svelte components. Use `$props()`.
5. Do not use `on:click` in new Svelte components. Use `onclick=`.
6. Do not use `$effect` for derived state. Use `$derived`.
7. Do not put server code (`*.server.ts`, `import { ... } from '$env/static/private'`, etc.) in `packages/features`, `packages/ui`, or `packages/shared`. Those packages are used by `apps/native` which has no server.
8. Do not import Tauri APIs (`@tauri-apps/*`) in `packages/ui` or `packages/features`. Tauri imports belong only in `apps/native/src/lib/native/`.
9. Do not enable broad Tauri permissions. Add to `src-tauri/capabilities/` only what an implemented feature actually uses.
10. Do not create a second SDK client in components. Call `getSdkContext()` from `@epifly/features`.
11. Do not use one SvelteKit config for both apps. `apps/web` uses `adapter-auto`; `apps/native` uses `adapter-static` with SPA fallback.
12. Auth is OIDC via Zitadel (Plan v5.1). **Web:** the browser holds only an opaque httpOnly session cookie (`__Host-epifly_sid`); the SvelteKit BFF holds the encrypted token set in Postgres and injects `Authorization` server-side — the browser never sees a token. **Native:** tokens live in the OS keychain behind a Rust token manager; JS never holds the refresh token. Never put tokens in `localStorage` / `sessionStorage` / IndexedDB / `UserDefaults` or in a cookie payload; never ship a client secret in any bundle; never open IdP pages in an in-app WebView.
13. Do not build custom sidebar primitives before using the shadcn-svelte Sidebar component.
14. Do not create folders for features that have no implemented code yet.
15. Do not branch UI or SDK adapters on storage `kind`/`mime_type` to tell files from conversations. Branch on `semantic_kind`.
16. Do not surface engineering terms (`projection`, `semantic_kind`, `source_id`) in the UI. Use user-facing vocabulary (Conversation, Document, Context, Paused, Restore, Move to…).

## Svelte 5 coding patterns

### Props

```svelte
<script lang="ts">
  type Props = {
    disabled?: boolean;
    onSubmit?: (value: string) => void | Promise<void>;
  };
  let { disabled = false, onSubmit }: Props = $props();
</script>
```

### Events

```svelte
<button onclick={handleClick}>
<form onsubmit={handleSubmit}>
```

### Derived state

```ts
let canSend = $derived(message.trim().length > 0 && !isStreaming);
```

### Effects — only for side effects

Use `$effect` only for: focus management, scroll-to-bottom, external subscriptions, analytics, DOM measurement, native bridge setup. Never for state derivation.

### File naming

- Kebab-case filenames: `chat-composer.svelte`, `threads.store.svelte.ts`
- `.svelte.ts` only when the file uses Svelte runes
- Plain `.ts` for non-rune utilities: `chat.utils.ts`, `platform.ts`
- PascalCase imports: `import ChatComposer from "@epifly/ui/components/chat/chat-composer.svelte"`

## Package responsibilities

### `packages/sdk`

The SDK is the only place that touches the network. It exposes:

```ts
createConusSdk({ baseUrl, tokenProvider, fetch })
```

Key modules: `auth`, `capabilities`, `chat`, `chatApi`, `files`, `realtime`, `shells`, `threads`, `ui`, `workspaces`. The endpoint map `EP` in `endpoints.ts` is the single source for all URL paths.

The `ApiResult<T>` pattern is used throughout:

```ts
type ApiResult<T> =
  | { data: T; error: null }
  | { data: null; error: ApiError };
```

Do not throw from UI-facing feature code. The SDK's `call` helper already converts failures into `{ data: null, error }`.

### `packages/features`

Runtime-neutral rune stores and actions. Gets the SDK via `getSdkContext()`. Never imports from `$app/` server modules or Tauri.

SDK context access:

```ts
import { getSdkContext } from "@epifly/features";
const sdk = getSdkContext();
```

Store files use `.svelte.ts` because they contain runes. Actions and utils use plain `.ts`.

### `packages/ui`

Two layers:

```
src/components/ui/          shadcn-svelte primitives (button, textarea, sidebar…)
src/components/app/         AppShell, AppSidebar, AppMobileHeader, AppMain, AppSafeArea
src/components/chat/        ChatComposer, ChatThread, ChatMessage, ChatMessageList…
src/components/workspace/   WorkspaceTree, WorkspaceSwitcher, WorkspaceNodeRow
src/components/account/     AccountMenu, UserAvatar
src/styles/                 tokens.css, motion.css
src/utils/cn.ts
```

Components here receive data via props and emit via callback props. They do not import from `packages/features` or `packages/sdk`. They do not call `getSdkContext()`.

### `apps/web`

SSR-capable SvelteKit app. Can use `+page.server.ts`, `hooks.server.ts`, and server-side auth. Wraps with `SdkProvider` using `createWebTokenProvider()`.

Routes:

```
(auth)/login/
(app)/+layout.svelte       — app shell with sidebar
(app)/+page.svelte         — root chat / new conversation
(app)/chat/[threadId]/     — existing thread
(app)/workspaces/
(app)/settings/
```

### `apps/native`

Static SPA inside Tauri v2. No SSR. Config:

```ts
// svelte.config.js
adapter: adapter({ pages: "build", assets: "build", fallback: "index.html" })

// src/routes/+layout.ts
export const ssr = false;
export const prerender = false;
```

Tauri-specific code lives in `src/lib/native/`:

```
platform.ts       Platform detection
token-provider.ts Native token provider (reads from secure storage)
safe-area.ts      Safe area inset helpers
window.ts         Window management helpers
```

Tauri config at `src-tauri/tauri.conf.json`:
- `devUrl`: `http://localhost:1420`
- `frontendDist`: `../build`
- `identifier`: `com.epifly.app`

Capabilities at `src-tauri/capabilities/`:
- `default.json` — `core:default` only
- `desktop.json` — desktop-only additions
- `mobile.json` — mobile-only additions

## Chat streaming

`sdk.chat.stream()` is an async generator that yields typed deltas:

```ts
for await (const delta of sdk.chat.stream({ message, threadId, workspaceNodeId, signal })) {
  switch (delta.kind) {
    case "text":             // append to assistant message
    case "thread_id":        // capture new threadId
    case "tool_start":       // show tool event row
    case "tool_result":      // update tool event row
    case "routing_meta":     // show routing info
    case "resource_invalidated": // invalidate cached data
    case "done":             // mark message complete
  }
}
```

Stop streaming with `abortController.abort()`. The signal is passed into `sdk.chat.stream`.

## File uploads

Three upload paths exist in the SDK — use named actions, never call them ad hoc from components:

```ts
// packages/features/src/files/files.actions.ts
uploadWorkspaceFile(file)    // workspaces.upload → EP.UI_UPLOAD
uploadUiAttachment(file)     // ui.upload → EP.UI_UPLOAD
uploadPersistentFile(file)   // files.upload → /v1/files
extractInvoice(fileId)       // ui.extractInvoice → EP.UI_EXTRACT_INVOICE
```

## Realtime and shells

Never open websocket connections directly in components.

```
sdk.realtime.subscribe()         — /api/realtime/workspace, with reconnect + backoff
sdk.shells.control(deviceId)     — /v1/shells/{deviceId}/control
```

Wrap in feature stores. Close in `$effect` cleanup. Do not enable shell controls in mobile unless the feature is actually implemented.

## Styling

- Tailwind utilities for layout and spacing in components
- CSS variables from `tokens.css` for design tokens
- `motion.css` for animation timing

Motion constraints:
- Movement under 8 px
- Duration 120–240 ms
- No animated blobs
- No glassmorphism abuse
- Focus states must be visible
- Hover states must not be required on mobile (mobile has no hover)

## Implementation status (May 2026)

### Frontend — done
- Phase 1: All packages, pnpm workspace, TypeScript paths
- Phase 2: All UI components, styles, primitives
- Phase 3: SDK provider and context in both apps
- Phase 4: Chat store with full streaming, delta handling, stop, errors
- Phase 5: Thread store + routes; sidebar wired to `createThreadsStore()` (real recents)
- Phase 6: Workspace store + tree wired to `createWorkspacesStore()` (real nodes, lazy children, search, create)
- Phase 7: Native hardening — adapter-static, SSR off, capabilities, safe-area
- Chat pages in both apps integrate `createChatStore()` end-to-end (verified on the iOS simulator)
- Responsive/layout pass: `--toggle-bar-height` token, `--safe-left/right` defaults, keyboard-resize
  viewport meta, scroll anchoring (no glassmorphism)

### Backend — done
- agent-gateway/workspaces refactor Phases 0–5 complete (typed AgentMessage, parking_lot registry,
  workspace module split + durable indexing + object-key migration, provider abstraction + prompt
  hooks + property tests, WorkspaceNodeKind/thread_projections/ProjectionRedactor/tags/ThreadRuntime)
- `ThreadProjectionStore` has a factory (`build_thread_projection_store` / `ProjectionStoreBackend`)
  plus a shared contract-test suite (InMemory + redb-in-memory backend)

### Open gaps / active work
1. **Real auth (active roadmap — Plan v5.1).** Replace the dev HS256 `/v1/auth/login` with a real
   Zitadel/OIDC flow on web (SvelteKit BFF) and native (Tauri 2 / iOS). See `docs/plan.md`. Interim
   state today: `createWebTokenProvider()` reads a `sessionStorage` stub (with `setWebAccessToken` /
   `clearWebAccessToken` exports) and `createNativeTokenProvider()` returns `null`. The plan deletes
   the web `sessionStorage` path (Phase 2 → BFF cookie) and gives native its own keychain-backed
   provider (Phase 5).
2. **Workspace ⇄ Chat unification (Plan v4.1) — shipped.** Chat conversations are first-class nodes
   in the workspace tree; the load-bearing fix (SDK adapter branching on `semantic_kind`) is in place.
   The v4.1 UX invariants below now describe shipped behavior, not pending work.

## Running the apps

```bash
# Web dev server (port 5173)
pnpm --filter web run dev

# Native dev server (port 1420) + Tauri desktop
pnpm --filter native run tauri:dev

# iOS simulator (iPhone 16 Pro)
cd apps/native && pnpm tauri ios dev "iPhone 16 Pro"

# Android emulator
cd apps/native && pnpm tauri android dev
```

The web preview server is configured in `.claude/launch.json` as `web` (port 5173) and `browser-shell` (port 5174).

## Backend

Rust/Axum at `apps/backend`. Workspace `Cargo.toml` at repo root. The native crate is at `apps/native/src-tauri` (member `apps/native/src-tauri` in workspace). Do not reference `apps/browser-shell` — that path was renamed to `apps/native`.

Rust toolchain: 1.95 (pinned via `rust-toolchain.toml`). iOS target: `aarch64-apple-ios-sim`. Android targets: `aarch64-linux-android`, `armv7-linux-androideabi`.

## Active plan — docs/plan.md (Plan v5.1: Zitadel/OIDC end-to-end auth)

The authoritative roadmap is [docs/plan.md](docs/plan.md). The frontend Workspace-as-Spatial-Memory
plan (v4.1) and the backend agent-gateway/workspaces refactor (v3.x) are **complete and shipped** —
their invariants below remain load-bearing. `docs/plan.md` now holds **Plan v5.1**: replace the dev
HS256 `/v1/auth/login` with a real Zitadel/OIDC flow on the SvelteKit web app (BFF) and the Tauri 2
native app (iOS, plus macOS/Windows/Android for free) — register → email verify → sign-in →
JWKS-verified gateway access → silent refresh → sign-out everywhere. Standards-only (RFC 9700 BCP,
PKCE-S256, BFF for web, system browser for native). Treat it as the single source of truth — do not
duplicate its contents in commit messages or other docs.

The plan ships in linear phases (Z → 9) across five parallel tracks that converge at Phase 8:
**A** — IdP/Zitadel setup, **B** — backend JWKS verifier + tenant context + legacy removal,
**C** — web BFF (login/callback/logout, session store, reverse proxy, route guards),
**D** — native (system browser + deep-link, Rust token manager, keychain, refresh rotation),
**E** — security acceptance (token-storage / CSRF / cross-tenant / no-secret-in-bundle proofs, scans).
**Phase Z (Zitadel bootstrap, Z.1–Z.10) is a hard gate: do not start Phase 0 until all of Z is green.**

### How to execute the plan

**One step at a time. One concern per phase.** Pick the next unchecked step from the plan's Execution
checklist and state the contract in one paragraph (files, behavior delta, the test that proves it)
before coding. The plan's "Iteration protocol for the implementing agent" is authoritative; the
essentials:

1. If the step is testable, **write the failing test first** (Phase 0 JWKS verify + alg/iss/aud
   negatives, Phase 1 single-flight refresh + callback replay, Phase 5 reuse-detection, etc.).
2. Implement directly with Write/Edit. Read-only exploration may be delegated to sub-agents; **code
   writes must not.**
3. Fast gate by surface:
   - Backend → `cargo fmt && cargo clippy -p <crate> --all-targets -- -D warnings && cargo test -p <crate>`.
   - Frontend → `pnpm --filter @epifly/ui exec svelte-check`, `pnpm --filter web check-types`,
     `pnpm --filter native check-types`, `pnpm --filter @epifly/features test`.
   - Compose/env changed → `docker compose config --quiet`.
4. Run the phase's **Verify** steps (web and/or iOS — skip a track only when the phase says N/A).
   Capture URL, status, console, network entries, SQL probe output. A finding == a fix, same phase.
5. Commit with the plan's body format (subject; `Phase <n>.<m> — <what changed>`; `Verified:`;
   `Deferred:`). Tick the checklist line. One step = one commit.

**Per-phase gate (mandatory before next phase):** all checklist items for the phase ticked; web + iOS
verification both green; `pnpm test:e2e:web` for any web-touching phase; the phase's reviewer checklist
re-checked for the slice it changed.

**Stop / scope triggers:** Zitadel unreachable or misconfigured → pause until Z.1–Z.9 fixed; any
verification step failing twice with the same root cause → escalate; any token-handling code without a
test → block; any new log call in `apps/backend/crates/agent-gateway/src/{routes,mw}/auth*` without an
explicit redaction review → block; a step needing 3+ unrelated files → split; a step needing changes
outside its declared phase scope → STOP and write an "unplanned scope" note before continuing.

**Pre-flight audit before resuming:** read the plan's "Current-state audit" table and tick what's
already real before writing anything — e.g. `ZitadelProvider` already does introspection with a `moka`
cache (Phase 0 adds JWKS as the *default* and reuses both); `legacy.rs` HS256 verifier and the
`/v1/auth/login` route still exist (deleted in Phase 9); `token-provider.ts` still has the
`sessionStorage` path (deleted in Phase 2). Verify route/param/env shapes before assuming they exist.

### Backend architecture invariants (from the completed v3.x refactor — still load-bearing)

These describe the shipped backend. They remain true and must not be regressed:

1. **Onboarding is a misnaming bug, not a logic bug.** Step 0.1 renames; never invert the condition.
2. **Never use `str::starts_with` for path containment.** Use `VirtualPath::is_strict_child_of` for child uploads, `is_same_or_within` for content routes. `VirtualPath` constructors must be private — the security boundary is `parse`.
3. **Never retry an LLM call after the first response byte.** Carry `request_id` across retries (Step 1.3).
4. **Non-tool models:** for normal chat, force `tools = []`. Reject only when tools are actually required (`forced_capability` or `decision.tool_required == true` with an explicit `ToolRequirementReason`).
5. **Cancellation is a feature.** The async sink (Step 2.5) must propagate client-disconnect into the tool loop before the next tool call fires.
6. **`AgentEvent` stays typed end-to-end.** SSE/JSON encoding happens only at the sink boundary. Never push `Bytes` into the runner.
7. **Module direction:** `agent::* → routes::*` reverse import is allowed only transitionally in Step 2.1. By Step 2.7 the direction is inverted; CI must enforce.
8. **Best-effort cleanup uses `tokio::join!`, not `try_join!`.** `try_join!` short-circuits and defeats the purpose.
9. **Storage migration is dual-read / dual-write / backfill / cutover.** Never "copy keys and switch." New key is primary; legacy is best-effort.
10. **Indexing jobs check `content_version` before upserting.** Stale-write races are the default failure mode.
11. **Thread projection is durable, not `tokio::spawn`.** Use `ThreadProjectionJob` with coalescing per `(tenant, thread)` and boot-time reclaim of stuck `running` jobs.
12. **Files and threads share infrastructure, not identity.** Distinguish via `semantic_kind` (`WorkspaceNodeKind::Thread`), never via the storage `kind`/`mime_type`. The UI **and the SDK adapter** branch on `semantic_kind` — branching on storage `kind` is exactly the bug Plan v4.1 Step 0.1 fixes.
13. **Delete of a thread node = pause projection, not redb delete.** Never silently resurrect a deleted projection node on the next turn. UI shows `[Restore]` instead.
14. **`RedactPiiHook` ≠ `ProjectionRedactor`.** Hook is logs/audit (opt-in for prompts, prohibited for tool args). Redactor is mandatory for the MD body and search payload; bypass only via test-only `unsafe_unredacted()`.
15. **`ThreadRuntime` is a performance layer, never the source of truth.** Every `AgentEvent::Done` must be preceded by a successful synchronous `append_message` — assert this with a property test (Step 5.7).
16. **`agent-core` over `agent-gateway`** when placement is ambiguous — `agent-core` is testable without HTTP.
17. Preserve existing routing audit fields when refactoring; observability is already good. Do not regress it.

### Frontend / workspace-UX invariants (Plan v4.1 — shipped; still load-bearing)

Product model: **a workspace is spatial memory, not a folder tree.** A conversation is a living
document you talk to — one `node_id`, openable as chat, readable as a document, living in one place.
Every surface should answer the user's five questions: *where am I, what am I working on, what does
the assistant know here, where will this save, how do I find it later.*

18. **Branch on `semantic_kind`, never storage `kind`/mime** — in the SDK adapter and the UI (see #12).
19. **One `node_id`, every lane.** Recents, Tree, Smart Views, and search render the *same* node;
    selecting in one reflects in all. Never fork identity between a "conversation" and its "node".
20. **Tree order is user-owned — never auto-sort it.** Recency lives only in the Recents lane.
21. **Delete of a conversation = pause (`hidden_at`), never destroy.** Always offer `[Restore]`.
22. **Suggest, never silently act.** The system may propose a folder; the user confirms. No silent
    auto-move; placement and order stay user-owned.
23. **Nothing is orphaned.** Every conversation has a visible home (a folder or the **Unsorted**
    view) from the moment it exists.
24. **Context is visible.** When ambient context is used, the UI names the place ("Using context from …").
25. **Optimistic, never blocking.** Projection is async; show a "syncing" affordance, never block
    chat or the tree.
26. **Keep engineering terms out of the UI** (see rule 16). Users see *Conversation / Workspace /
    Document / Context / Paused / Restore / Move to / View as document*.
27. **Every pointer action has a keyboard path.** Move/rename/delete/pause/restore reachable via menu
    + Cmd+K; drag-and-drop is never the only path.
28. **No graph UI.** The relationship/memory layer is backend intelligence for search, related-items,
    and suggestions — never a visual graph canvas.

### Auth invariants (Plan v5.1 — load-bearing; violating any invalidates the security model)

29. **No token reaches browser-readable storage or a cookie payload.** Web tokens live only in
    `auth_sessions.*_ct` (AEAD-encrypted in Postgres), decrypted per-request inside the BFF. The
    `__Host-epifly_sid` cookie carries an opaque session id and nothing else (no `sub`, no `email`).
30. **Native tokens live only in the OS keychain, managed in Rust.** The WebView never touches the
    refresh token; access tokens are handed out per-request via a Tauri command; no JS module caches them.
31. **JWKS local validation is the default verifier; introspection is opt-in.** Gated by
    `ZITADEL_TOKEN_VERIFY_MODE` (`jwks` default | `introspection`) or a `RequireIntrospection`
    extractor for revocation-sensitive routes. The two paths never silently mix.
32. **`alg` comes from a server-side allowlist (`{ RS256 }`), never the token header.** Reject `none`
    and HS256-confusion. `iss` is exact-string equality; also verify `aud`, `exp` (60s skew), `nbf`,
    `iat` skew, and non-empty `sub`. Unknown `kid` triggers exactly one single-flight JWKS refresh.
33. **PKCE-S256 only.** No implicit / hybrid / password grants. No client secret in any web bundle or
    native binary. Discovery is validated against the configured issuer and fails closed on mismatch.
34. **`state` is stored server-side and consumed exactly once** (replay → 400); `nonce` validated on
    the ID token. `returnTo` is allowlisted server-side. The session id is rotated after callback
    (fixation prevention).
35. **Refresh is single-flight.** Web: per `auth_sessions.id` via `SELECT … FOR UPDATE`, re-checking
    `access_expires_at` after the lock. Native: mutex + notify, proactive (60s before expiry) plus one
    401-triggered retry. Rotation is atomic; `invalid_grant` clears session/keychain and forces
    re-login (reuse-detection), best-effort revocation never blocks cleanup.
36. **Native uses the system browser only** (`opener`, never `inAppBrowser`). Universal/App Links are
    the production redirect; custom scheme is fallback. The deep-link handler validates scheme + host +
    path + state + exact `redirect_uri` + transaction age, and subscribes via **both** `get_current()`
    (cold start) and `on_open_url()` (runtime) — missing one drops callbacks silently.
37. **Tenancy is derived from the JWT only** — `org_id` → tenant, `(iss, sub)` → user; never `email`.
    Inbound `X-Tenant-ID` is rejected at the edge in production (`400 tenant_header_forbidden`) and the
    BFF strips it. Silent tenant auto-creation is forbidden in prod (`AUTH_AUTO_PROVISION_TENANTS=false`,
    CI-enforced).
38. **Auth logs redact `code`, all tokens, cookies, `email`, and raw claims** — no token/code/claims
    body in any log call. Dev-auth (`dev-auth` feature / dev profile) cannot start in a production
    build (CI gate).
