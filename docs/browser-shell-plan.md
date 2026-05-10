# Browser Shell ↔ ConusAI Web Parity Plan

**Status:** approved · 2026-05-10 (revised after expert review)
**Owners:** platform / frontend
**Related:** [docs/tasks/app.md](tasks/app.md), [docs/adr/006-tauri-browser-shell.md](adr/006-tauri-browser-shell.md), [docs/adr/008-multi-platform-shell.md](adr/008-multi-platform-shell.md)

## 1. Goal

Ship the **Tauri browser-shell** as the desktop ConusAI app: same chat, workspace, auth, theming, and assets as `apps/web`, **plus** the shell-only superpowers (tab management, session recorder, Stronghold-backed device tokens, screenshot capture).

Today, [apps/browser-shell](../apps/browser-shell) only consumes a thin `@conusai/ui` widget set; it has no Foundry CSS, no logos/favicons, no chat composer, no workspace dialogs, no auth, no API client. The audit in §2 lists the deltas; §5–§10 implement the fix.

## 2. Current state vs. target

| Surface | `apps/web` | `apps/browser-shell` | Action |
|---|---|---|---|
| Foundry CSS (Paper/Forge, 1,526 lines) | ✅ [app.css](../apps/web/src/app.css) | ❌ | Promote to `@conusai/ui` |
| `data-theme` flash-prevention + toggle | ✅ [app.html](../apps/web/src/app.html) | ❌ | Port to shell `app.html` |
| Web fonts (Fraunces / Switzer / JetBrains Mono) | ✅ remote CDN | ❌ | Self-host in `@conusai/ui` |
| Logos, favicon, icon sprite | ✅ [static/](../apps/web/static) | ❌ | Move to `@conusai/ui/assets` |
| Streaming chat composer + tool cards | ✅ 968 LoC in [+page.svelte](../apps/web/src/routes/+page.svelte) | ❌ | Extract to `@conusai/ui` features |
| Workspace tree (lazy load, search, dialogs) | ✅ | partial (read-only `WorkspaceTree`) | Promote dialogs + search |
| Auth (login, session, hooks) | ✅ [hooks.server.ts](../apps/web/src/hooks.server.ts) | ❌ | Replace SvelteKit server with Stronghold-token client auth |
| API client (`lib/api/*`) | ✅ | ❌ (uses Tauri `invoke` only) | Promote to `@conusai/sdk` |
| Toasts, LiveAnnouncer, a11y actions | ✅ | partial (`ToastHost`) | Promote `LiveAnnouncer` + actions |
| Tabs / recorder / Stronghold | ❌ | ✅ | Keep shell-only |

## 3. Architecture decision

**Option B: shared `@conusai/ui` + dual SvelteKit apps**, *not* embedding the web SPA in a WebView.

Rationale:
- Embedding `apps/web` (Option A in [docs/tasks/app.md](tasks/app.md)) locks the desktop UX to the gateway origin, forces cookie-based login, and marshals recorder/tab events across an iframe boundary — fragile and adds a network hop per interaction. Every major agent platform (Claude Desktop, Cursor, Windsurf) eventually regrets the iframe path.
- A shared `@conusai/ui` package, consumed by both `apps/web` (adapter-node, served by the gateway) and `apps/browser-shell` (adapter-static, packaged by Tauri), keeps each adapter idiomatic, lets the shell call `invoke()` directly, and gives the gateway the canonical web build for free.
- Cost: one refactor pass to lift web code into the shared package. Benefit: one source of truth, no iframe, mobile shell ([adr/008](adr/008-multi-platform-shell.md)) reuses everything.

## 4. Cross-cutting standards (apply in every phase)

These are non-negotiable 2026 conventions; new code that violates them is rejected at PR review.

### 4.1 Canonical package layout

`@conusai/ui` enforces this exact tree (matches the dominant 2026 Svelte 5 design-system convention):

```
packages/ui/
├── src/
│   └── lib/
│       ├── components/        ← presentational primitives (ThemeSwitcher, ToastHost, …)
│       ├── features/          ← feature slices (AgentChatStream, WorkspaceExplorer, …)
│       ├── stores/            ← runes-based stores (themeStore, modeStore, featureFlags)
│       ├── utils/             ← liveAnnouncer, focus-trap helpers
│       ├── assets/
│       │   ├── fonts/
│       │   └── icons/
│       └── index.ts           ← explicit barrel; no wildcard re-exports
├── tests/                     ← vitest + @testing-library/svelte
└── package.json               ← "exports" map + "svelte": "./src/lib/index.ts"
```

No `routes/` inside `@conusai/ui`. Routes live in the two apps. Any pre-existing `lib/ui/`, `lib/workspace/` paths in `apps/web` are deleted as code is promoted.

### 4.2 Svelte 5 runes everywhere
All new and promoted components **must** use runes (`$state`, `$derived`, `$effect`, `$props`). Hooks become factory functions: `createChatStream()` returns a `$state` object instead of a `useChatStream()` hook. Legacy `let`-reactivity is rejected at review.

### 4.3 Build caching
Adopt Turborepo at monorepo root via [turbo.json](../turbo.json) — extend pipelines for `@conusai/ui`, `@conusai/sdk`. Cuts `pnpm --filter browser-shell build` from ~45 s → ~12 s after the first run because shared packages become cacheable artefacts.

### 4.4 Unified Playwright config
A single `playwright.config.ts` at monorepo root with two projects:
- `web` — chromium against `pnpm --filter web preview`
- `browser-shell` — `@tauri-apps/cli` webdriver protocol against `pnpm tauri dev`

Replaces per-app test setups; lets the same `test('parity: chat round-trip')` run on both targets.

### 4.5 Naming conventions (canonical)

| Old / draft name | Canonical |
|---|---|
| `ChatStream` | `AgentChatStream` (matches Rust `agent-core`) |
| `WorkspaceSidebar` | `WorkspaceExplorer` (matches Finder/VS Code + ADR 006) |
| `ThemeToggle` | `ThemeSwitcher` |
| `useChatStream()` (hook) | `createChatStream()` (rune factory) |
| `useWorkspaceTree()` | `createWorkspaceTree()` |
| SDK entry `createClient` | `createConusSdk` (matches `createOpenAI`, `createAnthropic`) |

### 4.6 Workstream layout

Six sequenced phases. Each is independently mergeable, ends with a verification step (web smoke + shell smoke), and leaves both apps green.

```
P1 Shared assets & icons     →  visual parity unblocked
P2 Shared design system      →  Foundry CSS + self-hosted fonts in @conusai/ui
P3 Shared API client / SDK   →  one createConusSdk(), no duplicated fetch code
P4 Shared chat + workspace   →  largest lift; runes-only feature slices
P5 Shell auth (Stronghold)   →  replaces SvelteKit hooks with native auth
P6 Shell integration         →  wires recorder/tabs around the shared UI
```

## 5. Phase 1 — Shared assets & favicon

**Goal:** every logo, icon, and favicon lives in one place; both apps reference it.
**Effort:** ~1 AI-hour.

1. Create `packages/ui/src/lib/assets/` and move from [apps/web/static/](../apps/web/static):
   - `images/conusai-logo-lightmode.png`
   - `images/conusai-logo-darkmode.png`
   - `images/favicon.png`
   - `icons/icons.svg`
2. Add an export map in `packages/ui/package.json`:
   ```json
   "exports": {
     ".": { "svelte": "./src/lib/index.ts", "types": "./src/lib/index.ts" },
     "./assets/*": "./src/lib/assets/*",
     "./tokens.css": "./src/lib/tokens.css",
     "./foundry.css": "./src/lib/foundry.css"
   }
   ```
3. In `apps/web`, replace `static/` references with imports (`import logo from '@conusai/ui/assets/conusai-logo-lightmode.png'`) so Vite fingerprints them. Reference favicon via `<link rel="icon" href={favicon}>` in `+layout.svelte`.
4. In `apps/browser-shell`, add `vite-plugin-static-copy` ^1.x to copy **only** the files referenced by the export map into the build output — no manual `static/` folder.
5. Regenerate Tauri bundle icons from the canonical light-mode logo (`pnpm tauri icon packages/ui/src/lib/assets/conusai-logo-lightmode.png`); replace [apps/browser-shell/src-tauri/icons/](../apps/browser-shell/src-tauri/icons).

**Verify:** `pnpm --filter web build && pnpm --filter browser-shell build`; visually confirm favicon and sidebar logo render in both `vite preview` and `pnpm tauri dev`.

## 6. Phase 2 — Shared design system

**Goal:** one CSS source of truth (`foundry.css`), one font strategy, dark/light theme available everywhere.
**Effort:** ~2 AI-hours.

1. Move [apps/web/src/app.css](../apps/web/src/app.css) → `packages/ui/src/lib/foundry.css`. Keep `tokens.css` as a re-export (`@import './foundry.css';`) for backwards compatibility.
2. **Self-host fonts.** Subset Fraunces, Switzer, JetBrains Mono into `packages/ui/src/lib/assets/fonts/` with `font-display: swap` and explicit `font-feature-settings` for Fraunces variable axes. This:
   - removes the Google/Fontshare runtime dependency (Tauri offline + CSP friendly),
   - eliminates font-flash,
   - shrinks shell CSP to `default-src 'self'` (drop `connect-src https:` exception currently in [tauri.conf.json](../apps/browser-shell/src-tauri/tauri.conf.json)).
3. Add `:root { --foundry-version: "2026.05"; }` to `foundry.css` for runtime introspection (used by debug menus and telemetry).
4. In `apps/web/src/app.html`, delete the four `<link rel="preconnect">` + Google/Fontshare `<link>` tags. Import `@conusai/ui/foundry.css` from the root `+layout.svelte` exactly once.
5. In `apps/browser-shell/src/app.html`, add the `data-theme` attribute and the flash-prevention `<script>` from [apps/web/src/app.html](../apps/web/src/app.html#L8-L17).
6. Promote the theme toggle (currently inlined in `apps/web/+page.svelte`) into `packages/ui/src/lib/components/ThemeSwitcher.svelte`. Back it with `packages/ui/src/lib/stores/themeStore.ts` — a runes-based store with two adapters selected at app boundary:
   - web: `localStorage`
   - shell: `tauri-plugin-store`
7. Add a tiny `ThemeProvider.svelte` (runes) that wraps each app's `+layout.svelte` and emits a Tauri event `theme-change` so Rust can re-tint the macOS tray icon when needed.
8. Both apps' root `+layout.svelte` ends up with: `import '@conusai/ui/foundry.css'; <ThemeProvider>{@render children()}</ThemeProvider>`.

**Verify:**
- `apps/web`: paper ↔ forge toggle still works, no FOUC.
- `apps/browser-shell`: launch in `pnpm tauri dev`, confirm Fraunces renders offline (DevTools Network → fonts come from `app://`); theme toggle works; tray icon recolours on theme change.

## 7. Phase 3 — Shared API client & types

**Goal:** one TypeScript SDK for all REST/SSE calls. Web and shell pick different transports but the same surface.
**Effort:** ~3 AI-hours.

1. Audit [apps/web/src/lib/api/](../apps/web/src/lib/api): `client.ts`, `endpoints.ts`, `glyphs.ts`, `stream.ts`, `types.ts`, `workspaces.ts`. Pure ESM; only SvelteKit dep is the `fetch` argument.
2. Move all of `lib/api/` into `packages/sdk/src/`.
3. Replace the `fetch` parameter with a constructor-injected fetch implementation, exposing a single named factory `createConusSdk`:
   ```ts
   export function createConusSdk(opts: ClientOpts) {
     const client = createInternalClient(opts);
     return {
       workspaces: workspacesApi(client),
       chat:       chatApi(client),
       threads:    threadsApi(client),
       files:      filesApi(client),
     } as const;
   }

   interface ClientOpts {
     fetch: typeof globalThis.fetch;
     baseUrl: string;
     tokenProvider: TokenProvider;
   }
   ```
4. In `apps/web`, build the SDK per-request in `+layout.server.ts` using SvelteKit's `event.fetch` + cookie-based `TokenProvider`.
5. In `apps/browser-shell`, build the SDK once in `+layout.svelte` using `globalThis.fetch` + the Stronghold-loaded `TokenProvider` (Phase 5).
6. Move shared response shapes from [apps/web/src/lib/api/types.ts](../apps/web/src/lib/api/types.ts) into `@conusai/types`. Generate the rest from OpenAPI via [scripts/openapi-to-types.sh](../scripts/openapi-to-types.sh) wired into the `turbo build` pipeline so it runs on every build.
7. Re-export from `@conusai/sdk`:
   ```diff
   - import { workspacesApi } from '$lib/api';
   + import { createConusSdk } from '@conusai/sdk';
   ```
8. Preserve the existing discriminated `{ data } | { error }` return union — no `throw` in SDK methods, no `any`.

**Verify:** move [tests/sse-parser.test.ts](../apps/web/src/tests/sse-parser.test.ts), [tests/reconnect.test.ts](../apps/web/src/tests/reconnect.test.ts) into `packages/sdk/tests/`; `pnpm --filter @conusai/sdk test` green; both apps build; web smoke passes.

## 8. Phase 4 — Shared chat + workspace features

**Goal:** the 968-line [apps/web/src/routes/+page.svelte](../apps/web/src/routes/+page.svelte) becomes a thin route (≤ 80 LoC) that composes shared, runes-only feature slices.
**Effort:** ~12 AI-hours (largest chunk; split across 2–3 PRs).

Decompose into `packages/ui/src/lib/features/`:

| Component / factory | Responsibility | Source LoC |
|---|---|---|
| `AgentChatComposer.svelte` | textarea, attachments, focus, autogrow | ~80 |
| `AgentChatStream.svelte` | message list, word-token animation, scroll-near | ~180 |
| `ToolCallCard.svelte` | running/success/error badge, expandable result | ~60 |
| `WorkspaceExplorer.svelte` | tree + lazy load + search + recents | ~250 |
| `NewNodeDialog.svelte`, `ConfirmDialog.svelte`, `MoveDialog.svelte`, `ShareDialog.svelte` | promote from [lib/workspace/dialogs/](../apps/web/src/lib/workspace/dialogs) as-is | — |
| `createChatStream()` (rune factory) | wraps `conusSdk.chat.stream`, AbortController, 45 s inactivity timer, tool-card map | ~120 |
| `createWorkspaceTree()` (rune factory) | tree state, lazy loading, search debounce, refresh | ~80 |

Steps:
1. Land each component as a pure copy-paste from `apps/web` into `packages/ui`, **converting class-style reactivity to runes** as the only behaviour change. Run `apps/web` after each move; nothing else should regress.
2. Move `lib/ui/toast.svelte.ts`, `lib/ui/actions.ts`, `lib/ui/LiveAnnouncer.svelte` into `@conusai/ui` (`stores/`, `utils/`, `components/` respectively).
3. Move `lib/workspace/context.svelte.ts` into `packages/ui/src/lib/stores/`.
4. Add `packages/ui/src/lib/stores/featureFlags.ts` (runes) so the shell can disable/enable slices (e.g. recorder UI affordances) without forking components.
5. Reduce `apps/web/src/routes/+page.svelte` to ≤ 80 LoC composing `WorkspaceExplorer`, `AgentChatComposer`, `AgentChatStream`.
6. **A11y guardrails** enforced while extracting:
   - All dialogs use `<dialog>` with `inert` siblings + focus-trap.
   - Streaming text uses `aria-live="polite"` via `LiveAnnouncer`.
   - Composer is a `<form>` with explicit submit; Enter sends, Shift+Enter newlines.
   - Tool cards expose status via `aria-label` on the badge, not colour alone.

**Verify:**
- Web: existing Playwright [e2e/smoke.test.ts](../apps/web/e2e/smoke.test.ts) still passes.
- New: `packages/ui/tests/` with vitest + `@testing-library/svelte` covering `AgentChatStream` word-flush, `WorkspaceExplorer` lazy load, `ToolCallCard` status transitions.
- 100% rune usage in promoted components (lint rule).

## 9. Phase 5 — Shell auth (Stronghold-native)

**Goal:** the shell does not need SvelteKit `hooks.server.ts` or cookie session; it authenticates via a device token loaded from Stronghold (already partially scaffolded in [apps/browser-shell/src/routes/+layout.svelte](../apps/browser-shell/src/routes/+layout.svelte#L36-L52)).
**Effort:** ~3 AI-hours (≈1 Rust + 2 TS).

1. Define a `TokenProvider` interface in `@conusai/sdk`:
   ```ts
   export interface TokenProvider {
     get(): Promise<string | null>;
     set(token: string): Promise<void>;
     clear(): Promise<void>;
   }
   ```
2. **Web** implementation: cookie-backed (`event.locals.session` server-side, `/v1/session` client-side). No change to existing flow.
3. **Shell** implementation in [apps/browser-shell/src-tauri/src/keychain.rs](../apps/browser-shell/src-tauri/src/keychain.rs) using the **Tauri v2 state pattern**:
   ```rust
   pub struct KeychainState(pub tokio::sync::RwLock<Stronghold>);

   #[derive(Debug, thiserror::Error, serde::Serialize)]
   pub enum KeychainError {
       #[error("vault not provisioned")] NotProvisioned,
       #[error("token missing")]         Missing,
       #[error(transparent)]             Stronghold(#[from] iota_stronghold::ClientError),
   }

   #[tauri::command]
   pub async fn get_device_token(state: tauri::State<'_, KeychainState>) -> Result<String, KeychainError> { … }

   #[tauri::command]
   pub async fn set_device_token(token: String, state: tauri::State<'_, KeychainState>) -> Result<(), KeychainError> { … }

   #[tauri::command]
   pub async fn clear_device_token(state: tauri::State<'_, KeychainState>) -> Result<(), KeychainError> { … }
   ```
   - All errors are `thiserror` + `serde::Serialize` (never `anyhow::Error` — that breaks Tauri TS type generation).
   - All inputs validated via serde at the command boundary.
4. On first launch, if `get_device_token` returns `Missing`, render `LoginPanel.svelte` (promoted from [apps/web/src/routes/login/+page.svelte](../apps/web/src/routes/login/+page.svelte) into `@conusai/ui/features/auth/`) with a pluggable `onSubmit(creds) => Promise<token>`.
5. After successful sign-in, call `set_device_token` (shell) or rely on cookie set by gateway (web).
6. Sign-out clears Stronghold (`clear_device_token`) + re-mounts `LoginPanel`. Web sign-out hits `/logout`.
7. Replace the env-var token fallback (currently end of `loadTokenFromStronghold`) with an explicit "Sign in" CTA — no silent fallback in production builds.

**Security checks (OWASP-aligned):**
- Stronghold passphrase derived from OS keychain (macOS Keychain / Windows Credential Manager / GNOME Keyring) via [tauri-plugin-stronghold](https://v2.tauri.app/plugin/stronghold/) — never hardcoded.
- Token never logged.
- Shell CSP in [tauri.conf.json](../apps/browser-shell/src-tauri/tauri.conf.json) tightened to: `default-src 'self'; connect-src 'self' https://api.<gateway> wss://api.<gateway>; img-src 'self' data: blob:`. Sourced from a single `packages/config/csp.ts` so web and shell stay in sync.
- `withGlobalTauri: false` (already set).

**Verify:** manual login → quit app → relaunch → no re-auth prompt; manual sign-out → relaunch → login screen shown; Tauri-generated TS types include `KeychainError` variants.

## 10. Phase 6 — Shell integration

**Goal:** wire shell-only features (tabs, recorder) around the now-shared chat/workspace UI without forking either.
**Effort:** ~4 AI-hours.

Final shell layout (`apps/browser-shell/src/routes/+layout.svelte`):

```
┌─ AppShell (sidebar slot) ────────────────────────────────┐
│  ┌─ WorkspaceExplorer (shared) ┐  ┌─ TabStrip (shell) ─┐ │
│  │ tree + search + recents     │  │ tab tabs           │ │
│  └─────────────────────────────┘  └────────────────────┘ │
│                                   ┌─ active panel ────┐  │
│                                   │  AgentChatStream  │  │
│                                   │  OR WebView tab   │  │
│                                   │  OR ArtifactPrev  │  │
│                                   └───────────────────┘  │
│  ┌─ RecorderControls (shell) ──┐                         │
│  └─────────────────────────────┘                         │
└──────────────────────────────────────────────────────────┘
```

1. Replace [apps/browser-shell/src/routes/+page.svelte](../apps/browser-shell/src/routes/+page.svelte) with the same composition `apps/web` uses, gated by `modeStore` (`@conusai/ui/stores/modeStore.ts`): `'chat' | 'tab' | 'trace'`.
2. **Tab content via Tauri v2.2+ `webview_window` API** — *not* DOM coordinate hacks.
   - Add a Tauri command `set_active_tab_content_bounds(rect: Rect)` in [tabs.rs](../apps/browser-shell/src-tauri/src/tabs.rs) that calls `webview.set_position` + `set_size`.
   - `TabStrip.svelte` calls it on `mode` change and on `ResizeObserver` events from the tab-content host element.
   - Removes all manual DOM math; no `<div data-tauri-tab>` stub needed.
3. Recorder: keep [recorder.rs](../apps/browser-shell/src-tauri/src/recorder.rs) untouched. The existing `ArtifactBridge` upload returns a workspace node id; the shell dispatches a `selectNode` event that `WorkspaceExplorer` and `AgentChatStream` consume.
4. Persist `mode`, `activeTabId`, `selectedNodeId` via [tauri-plugin-store](https://v2.tauri.app/plugin/store/) (already pulled in indirectly).
5. Delete any SvelteKit server hooks/routes from the shell — adapter-static does not run them.

**Verify (per [docs/verify/verify.md](verify/verify.md)):**
- `pnpm tauri dev` launches → `LoginPanel` → after login, `WorkspaceExplorer` shows live tree from gateway → "New conversation" → send message → streamed reply with same animation as web → start recorder → load page in tab → stop recorder → trace appears as workspace node → click trace → `ArtifactPreview` renders JSON.
- Unified Playwright suite: `web` + `browser-shell` projects both green for the parity smoke spec.

## 11. Cross-cutting concerns

- **Build perf.** `@conusai/ui` is pure ESM with `"sideEffects": ["**/*.css"]` so Vite tree-shakes unused components. Turborepo caches `@conusai/ui` and `@conusai/sdk` build outputs.
- **Type safety.** No `any` introduced; SDK calls preserve the discriminated `{ data } | { error }` return shape. Tauri commands surface typed `thiserror` enums to TS.
- **i18n.** Out of scope. Keep all copy English. Document string locations (composer placeholder, login labels, dialog titles) in a `STRINGS.md` for a future i18n pass.
- **Telemetry.** `track(event: TelemetryEvent, props)` helper in `@conusai/sdk` typed as a discriminated union so Rust backend and frontend emit identical event shapes. Shell continues to use [telemetry.rs](../apps/browser-shell/src-tauri/src/telemetry.rs) for system-level metrics.
- **CSP.** Single source in `packages/config/csp.ts`; both gateway and `tauri.conf.json` read it.
- **Versioning.** After Phase 4, bump `@conusai/ui` and `@conusai/sdk` to `0.6.0` (semver — public re-export surface changes). Apps pin to `workspace:*`.
- **Explicitly NOT adopted:** TanStack Query, Zustand, i18n libraries, virtualised lists, additional state managers. Plan stays minimal.

## 12. Rollout order & exit criteria

| Phase | PRs | AI-hours | Exit criterion |
|---|---|---|---|
| P1 assets | 1 | 1 | Both apps render correct logo + favicon from shared package |
| P2 design system | 1 | 2 | `app.css` deleted from `apps/web`; both apps theme-switch; offline fonts in shell |
| P3 SDK | 1 | 3 | `apps/web/src/lib/api` deleted; `createConusSdk` used in both apps; SDK tests green |
| P4 chat + workspace | 2–3 | 12 | `apps/web/+page.svelte` ≤ 80 LoC; 100% rune usage in promoted components; UI unit tests green |
| P5 shell auth | 1 | 3 | Shell sign-in / persistence / sign-out verified; typed errors in generated TS |
| P6 shell integration | 1 | 4 | Unified Playwright suite green; manual checklist signed off |
| **Total** | **6** | **25** | Desktop parity shipped |

Approximate token cost for LLM-assisted refactors: **~180k input / 45k output**, dominated by Phase 4.

## 13. Out of scope (intentionally deferred)

- Mobile shell ([adr/008-multi-platform-shell.md](adr/008-multi-platform-shell.md)) — same shared package will serve it later.
- Replacing the Askama Foundry UI ([docs/tasks/app.md](tasks/app.md)) — kept as zero-JS fallback / admin surface.
- Generated TypeScript client from utoipa — independent follow-up.
- Rewriting capability cards / artifact preview beyond what already exists in `@conusai/ui`.
- New state libraries, i18n, virtualisation. Re-evaluate only after parity ships.
