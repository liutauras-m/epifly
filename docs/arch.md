# ConusAI Platform - Architecture and Implementation Audit

> Current workspace audit date: 2026-05-11
> 
> This document is implementation-first. It reflects what is currently wired in source for:
> - apps/web
> - apps/browser-shell
> - packages/ui
> 
> Image assets were not reviewed for visual quality; only code usage and asset inventory were reviewed.

---

## 1. Platform Purpose and Structure

ConusAI is a pnpm + Cargo monorepo for a multi-tenant AI agent platform.

Core package purpose:

| Path | Purpose | Runtime |
| --- | --- | --- |
| `apps/backend` | Agent gateway, capabilities, jobs, API/UI serving | Rust (Axum) |
| `apps/web` | Public web workshop UI with SSR session bridge | SvelteKit (Node adapter) |
| `apps/browser-shell` | Cross-platform shell (desktop/mobile) with Tauri native bridge | SvelteKit + Tauri 2 |
| `packages/sdk` | Typed API client (`createConusSdk`) | TypeScript |
| `packages/types` | Shared domain contracts | TypeScript |
| `packages/ui` | Shared design system, components, features, stores, motion | Svelte 5 |

Design and behavior are intentionally centralized in `packages/ui` and consumed by both web and shell.

---

## 2. Implementation Patterns Used Across Web/Shell/UI

### 2.1 Shared UI-first composition

Common app structure in both web and shell:

1. Import `@conusai/ui/foundry.css` in root layout.
2. Wrap app in `ThemeProvider`.
3. Use UI features (`AgentChatComposer`, `AgentChatStream`, `WorkspaceExplorer`, etc.).
4. Use `createChatStream` from UI to unify streaming behavior.

### 2.2 Svelte 5 runes state model

State patterns are consistently rune-based (`$state`, `$derived`, `$effect`) in the shared UI and app features.

### 2.3 Registry pattern for capability renderers

`packages/ui/src/lib/capabilities/CapabilityRendererRegistry.ts` provides a runtime registry:

- register(name, renderer)
- unregister(name)
- get(card)

`apps/browser-shell/src/routes/+page.svelte` registers `trace.replay` renderer at startup.

### 2.4 Dual token model in shell

Shell auth path is split by concern:

- Device token from Tauri Rust via `invoke('get_device_token')`
- Session token for `/ui/*` in `x-session-token` header (cross-origin workaround for WKWebView cookie behavior)

### 2.5 Native-stream bridge pattern

`apps/browser-shell/src/lib/tauri-stream.ts` routes SSE through Rust events due WebKit buffering behavior.

---

## 3. Shared UI Package (packages/ui)

## 3.1 Purpose

`packages/ui` is the shared product surface for web and shell:

- design tokens
- global style system
- reusable primitives
- chat/workspace features
- motion utilities
- global stores
- capability renderer registry

## 3.2 Design system, colors, typography, brand

Primary token sources:

- `packages/ui/src/lib/tokens.css`
- `packages/ui/src/lib/foundry.css`

Theme model:

- `paper` (light)
- `forge` (dark)

Core palette:

- ink/paper neutrals (`--ink`, `--paper`, etc.)
- brand accent ember orange (`--ember`)
- secondary accent electric cyan (`--cyan`)
- semantic success/danger tokens

Brand and typography:

- Geist + Geist Mono self-hosted in `foundry.css`
- Brand assets are in `packages/ui/src/lib/assets/images`
- Logo variants and favicon assets are available for light/dark and color variants

## 3.3 Motion and animation system

Motion primitives:

- Spring interpolation: `motion/spring.ts`
- FLIP transitions: `motion/flip.ts`
- Stagger utility: `motion/stagger.ts`
- Tap interaction (ripple on Android, scale on other platforms): `motion/tap.ts`
- View transitions wrapper with fallback: `motion/viewTransition.ts`

Accessibility:

- Reduced motion respected (`prefers-reduced-motion`) in tokens and component-level animation guards.

## 3.4 Shared components and features implemented

Implemented shared primitives:

- `AppShell`
- `WorkspaceTree`
- `TabStrip`
- `RecorderControls`
- `ToastHost` + `LiveAnnouncer`
- `ThemeProvider` + `ThemeSwitcher` + `ThemeScript`
- `CommandPalette`
- `ArtifactPreview`

Implemented shared features:

- `AgentChatComposer`
- `AgentChatStream`
- `ToolCallCard`
- `WorkspaceExplorer`
- `createChatStream` state machine
- Workspace dialogs (`ConfirmDialog`, `MoveDialog`, `NewNodeDialog`, `ShareDialog`)
- `features/auth/LoginPanel`

## 3.5 Chat and workspace behavior in UI layer

`createChatStream.svelte.ts` implements:

- send/abort/new session lifecycle
- inactivity timeout guard (45s)
- thread id capture
- tool card lifecycle (`running` -> `success`/`error`)
- word-chunked streaming animation model
- thread load from `sdk.threads.messages`

`WorkspaceExplorer.svelte` implements:

- lazy folder tree loading
- search with debounce
- create folder/conversation
- selected node binding

## 3.6 Store model

Shared stores include:

- `themeStore`
- `featureFlags`
- `recentsStore`
- `breadcrumbsStore`
- `modeStore`
- `toast`

These stores are app-agnostic and reused across web/shell surfaces.

## 3.7 UI package implementation status

Implemented:

- Shared tokenized theme system
- Shared chat/workspace features used by apps
- Renderer registry and context wiring
- Global notification and accessibility announcer
- Motion utility library with fallback behavior

Partly implemented or currently underused:

- `CommandPalette`, `TabStrip`, `RecorderControls`, `ArtifactPreview`, `features/auth/LoginPanel` are implemented in `packages/ui` but not currently wired into `apps/web` main route or the current mobile shell route composition.
- `featureFlags` defaults (`recorder`, `tabs`, `traceReplay`) are present but not yet acting as a central runtime gate in current app flows.

---

## 4. Web App (apps/web)

## 4.1 Purpose

`apps/web` is the SSR workshop application for browser users.

Primary route surfaces:

- `/login` for session bootstrap
- `/` for workspace + chat
- `/logout` for session clear

## 4.2 Auth/session implementation

Session model is cookie-based:

- Cookie name: `conusai_session`
- Default issue/verify: local HMAC (`src/lib/server/session.ts`)
- Alternate adapter shape exists for backend JWT mode

Request pipeline:

- `hooks.server.ts` performs scoped CSRF checks and resolves `locals.user` by verifying cookie.
- `+layout.server.ts` redirects unauthenticated users to `/login`.

## 4.3 Main page implementation

`src/routes/+page.svelte` wires:

- `WorkspaceExplorer`
- `AgentChatComposer`
- `AgentChatStream`
- `ThemeSwitcher`
- `createChatStream(sdk)`

Interaction behavior:

- Node selection loads thread if metadata contains `thread_id`
- File upload goes through `sdk.workspaces.upload`
- `Cmd/Ctrl+N` starts new session
- Mobile sidebar slide-in behavior via CSS media query

## 4.4 Server data loading

`src/routes/+page.server.ts` loads in parallel:

- threads list -> recents
- capabilities list -> glyph/count shape
- workspace tree

The route returns safe defaults if any upstream call fails.

## 4.5 Web implementation status

Implemented:

- SSR login and session enforcement
- Workshop layout with sidebar/chat split
- Shared chat stream UI with tool cards
- Workspace browsing/search/create via shared UI feature
- Upload flow via SDK
- Theme switch and hydration marker

Partly implemented or inconsistent:

- `SessionAdapter` abstraction exists, but login action still directly calls local `sign(...)`; adapter swappability is only partly realized in route actions.
- Tests in `src/tests` target `../lib/api/stream`, but that source path is not present under `apps/web/src/lib`; this indicates test/source drift.
- Some web e2e assertions are smoke-level and do not deeply validate streaming/tool-card semantics end-to-end.

---

## 5. Browser Shell (apps/browser-shell)

## 5.1 Purpose

`apps/browser-shell` is the cross-platform shell runtime for desktop/mobile, with native bridges for:

- device authentication state
- stream transport workaround
- tab management
- session recording
- trace upload/registration

## 5.2 Shell UI implementation (Svelte)

Current route root:

- `src/routes/+page.svelte` sets shell mode and registers `trace.replay` capability renderer.
- Main UI surface is `MobileShell.svelte`.

`MobileShell.svelte` implements:

- local login card (name + plan)
- top bar + drawer + profile sheet
- three screens: chat, capabilities, artifacts
- chat through shared `createChatStream` with optional Tauri stream function
- local session persistence in `localStorage`
- session cookie issuance for gateway UI endpoints

Mobile screen modules:

- `screens/ChatScreen.svelte`
- `screens/CapabilitiesScreen.svelte`
- `screens/ArtifactsScreen.svelte`

## 5.3 Shell SDK and streaming behavior

`src/lib/sdk.ts`:

- resolves API base from build env
- injects `x-session-token` when available
- reads device token from Rust command provider

`src/lib/tauri-stream.ts`:

- starts native stream command
- listens for `chat:chunk:<id>` events
- yields unified chat deltas to UI feature layer
- supports abort

## 5.4 Tauri Rust core implementation

`src-tauri/src/lib.rs` wires:

- tab manager state
- recorder state
- device auth state
- stream registry
- commands for tabs/recorder/auth/stream/trace upload
- stronghold + dialog + http plugins

Implemented native modules:

- `tabs.rs`: create/navigate/close/list/save/restore tab summaries
- `recorder.rs`: start/stop/record/status with PII redaction heuristics
- `chat_stream.rs`: backend `/ui/stream` bridge -> Tauri events
- `device_auth.rs`: token provider/admin traits and command API
- `registration.rs`: startup capability registration and trace upload command

## 5.5 Browser-shell implementation status

Implemented:

- Mobile-first shell UI flow and drawer architecture
- Session persistence and shell-mode theme integration
- Native stream bridge for WebKit buffering constraint
- Device token state bridge with Stronghold integration points
- Native tab manager and recorder command surface
- Capability registration call on startup when token exists

Partly implemented or currently inconsistent:

- `TraceReplayCapability.svelte` invokes `upload_trace_cmd` with `{ traceNodeId, dryRun }`, but Rust command currently expects a full `trace: SessionTrace` payload. This path is not contract-aligned.
- `ArtifactsScreen.svelte` renders list rows but uses `onClick={() => {}}`; artifact open/preview action is not implemented.
- `MobileShell.svelte` keeps `workspaceNodes` state but does not currently hydrate it; `DrawerRecentChats` depends on `nodes`, so recent chat resolution is partial.
- `handleInvoke` in `MobileShell.svelte` switches to chat screen but does not execute/prefill capability invocation workflow.
- `capture_tab_screenshot` in `recorder.rs` intentionally returns an error until dependency/API support path is finalized (commented future code exists).
- Some shell e2e specs target selectors or surfaces from an older shell structure (for example `#name-input`, `.shell-content`, tab strip controls), indicating test-app drift.

---

## 6. Design and Brand Language (Code-level)

The codebase establishes a consistent brand language in UI code:

- Primary identity accent: ember orange
- Secondary technical accent: cyan
- Strong neutral substrate with dual light/dark tokens
- Geist typography throughout interface and mono data labeling
- Rounded geometry and soft/translucent surfaces

Motion language implemented in code:

- subtle spring and FLIP transitions
- staggered entry patterns
- sonar/ring status animations for thinking and loading states
- platform-sensitive tap affordances
- reduced-motion compliance

Brand assets and variants are centralized in `packages/ui/src/lib/assets/images` and consumed by both web and shell pages.

---

## 7. Feature Inventory Snapshot

### 7.1 Web

Implemented:

- Login/logout and cookie session flow
- Protected layout redirect
- Workspace explorer + chat canvas
- Streamed AI responses and tool cards
- Upload attachments through workspace endpoint
- Theme toggle and responsive sidebar behavior

Partial:

- Session adapter abstraction not fully routed through action handlers
- Test/source path mismatch around stream parser tests

### 7.2 Browser shell

Implemented:

- Local profile onboarding and session persistence
- Mobile drawer navigation and screen state stack
- Chat streaming through native bridge when in Tauri
- Capability listing/search and detail sheet
- Artifact list retrieval
- Native recorder/tabs/device auth command backplane

Partial:

- Artifact row actions
- Trace replay invoke contract mismatch
- Recent chat node hydration in drawer
- Capability invoke handoff into chat workflow
- Screenshot capture command blocked on version capability

### 7.3 Shared UI package

Implemented:

- Tokenized design system and theme infra
- Shared chat/workspace features used by both apps
- Renderer registry and feature stores
- Motion primitives and accessibility helpers

Partial/underused:

- Several reusable primitives exist but are not currently mounted in active app routes (`CommandPalette`, `TabStrip`, `RecorderControls`, `ArtifactPreview`, auth panel component)
- Feature flags are defined but not yet central to runtime flow gating

---

## 8. Testing Coverage State

Confirmed coverage in repository:

- Web smoke tests in `apps/web/e2e/smoke.test.ts`
- UI unit tests for stream state machine and capability registry in `packages/ui/tests`
- Web stream parser/reconnect tests exist in `apps/web/src/tests` but appear disconnected from current source path layout
- Cross-platform suites under top-level `e2e` include iOS simulation and shell macOS suites

Current risk:

- Several shell tests appear aligned to older selectors/layout contracts and likely need synchronization with current mobile-shell implementation.

---

## 9. Recommended Next Stabilization Steps

1. Fix trace replay contract mismatch between `TraceReplayCapability.svelte` and Tauri `upload_trace_cmd` command shape.
2. Implement artifact row action (preview/open/download) in `ArtifactsScreen.svelte`.
3. Hydrate/resolve workspace nodes for `DrawerRecentChats` so recents render correctly.
4. Complete capability invoke flow from capabilities screen into chat composer/dispatch.
5. Realign shell e2e selectors and scenarios with current route/component tree.
6. Resolve web stream test path drift (`apps/web/src/tests` vs actual stream implementation location).

---

## 10. Source Anchors Reviewed

Primary reviewed sources include:

- `apps/web/src/routes/*`
- `apps/web/src/lib/*`
- `apps/web/src/tests/*`
- `apps/web/e2e/*`
- `apps/browser-shell/src/routes/*`
- `apps/browser-shell/src/lib/**/*`
- `apps/browser-shell/src-tauri/src/*`
- `packages/ui/src/lib/**/*`
- `packages/ui/tests/*`
- `e2e/shell-macos/*`
- `e2e/ios/*`
