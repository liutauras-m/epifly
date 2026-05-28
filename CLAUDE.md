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
12. Do not store tokens in `localStorage` as the final native auth strategy.
13. Do not build custom sidebar primitives before using the shadcn-svelte Sidebar component.
14. Do not create folders for features that have no implemented code yet.

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

### Done
- Phase 1: All packages, pnpm workspace, TypeScript paths
- Phase 2: All UI components, styles, primitives
- Phase 3: SDK provider and context in both apps
- Phase 4: Chat store with full streaming, delta handling, stop, errors
- Phase 5: Thread store and routes; sidebar wiring is a stub (see gaps)
- Phase 6: Workspace store; tree wiring is a stub (see gaps)
- Phase 7: Native hardening — adapter-static, SSR off, capabilities, safe-area

### Open gaps
1. **Sidebar threads** — `app-navigation-sidebar.svelte` renders hardcoded placeholder history. Wire `createThreadsStore()` and replace the mock.
2. **Workspace tree** — same component renders a hardcoded folder tree. Wire `createWorkspacesStore()` and replace.
3. **Token providers** — both `createWebTokenProvider()` and `createNativeTokenProvider()` return `null`. Implement real auth once the auth flow is designed.
4. **Chat pages** — `+page.svelte` files in both apps are thin stubs. Integrate `createChatStore()` with the chat UI components.

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

## Backend refactor — agent gateway & workspaces (plan v3.4)

The authoritative roadmap is [docs/plan.md](docs/plan.md). It is six phases (0 → 5), step-numbered, and each step is independently executable. Treat it as the single source of truth — do not duplicate its contents in commit messages or other docs.

### How to execute the plan

**One step at a time. One concern per phase.** If a step requires touching code outside its declared phase scope, stop and write a short "unplanned scope" note before continuing (the plan's Stop condition, plan.md line 667).

**Per-step rhythm:**
1. Pick the next unchecked step from the plan's Execution checklist.
2. State the contract in one paragraph (files, behavior delta, the test that proves it) before coding.
3. Write the failing test first when the step is test-verifiable (0.3, 0.4, 1.1, 1.5, 2.3, 3.6, 4.4, 5.8). Pure renames skip — the compiler is the test.
4. Implement directly with Write/Edit. Do not delegate code writing to sub-agents (use them only for read-only exploration).
5. Run the fast gate: `cargo fmt && cargo clippy -p <touched> --all-targets -- -D warnings && cargo test -p <touched>`. Cross-crate → `--workspace`. Storage/migration → testcontainers.
6. Commit with a 3-line evaluation note in the body: *what changed*, *what's verified*, *what's deferred*.
7. Tick the plan's checklist line. One step = one commit, except where the plan calls out sub-commits (3.2a/b/c, 3.4, 3.5).

**Per-phase gate (mandatory before next phase):**
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `pnpm test:e2e:web` — never per-step, only at phase boundaries
- Phase-specific extras: Phase 1 → manual presign smoke; Phase 3 → testcontainer integration tests (RustFS + Postgres + Qdrant); Phase 4 → load test report; Phase 5 → rename/pause/restore smoke + secret-leak grep.
- Write a 5–10 line phase eval in the PR description: items closed, metrics now emitted, followups deferred.

**Pre-flight audit before resuming:** the tree already contains partial Phase 0/1/2 work (`agent-gateway/src/agent/` skeleton, `agent-core/src/model_catalog.rs`, Phase 0 test files). Before implementing anything, audit each phase's checklist against the actual code and tick the boxes that are already done. Do not re-implement them.

### Non-negotiable invariants (mirrored from plan.md "Notes for the executing agent")

These are load-bearing — violating them is the failure mode the plan was written to prevent:

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
12. **Files and threads share infrastructure, not identity.** Distinguish via `WorkspaceNodeKind::Thread`, never via `mime_type == "text/markdown"`. The UI branches on `kind`.
13. **Delete of a thread node = pause projection, not redb delete.** Never silently resurrect a deleted projection node on the next turn. UI shows `[Restore]` instead.
14. **`RedactPiiHook` ≠ `ProjectionRedactor`.** Hook is logs/audit (opt-in for prompts, prohibited for tool args). Redactor is mandatory for the MD body and search payload; bypass only via test-only `unsafe_unredacted()`.
15. **`ThreadRuntime` is a performance layer, never the source of truth.** Every `AgentEvent::Done` must be preceded by a successful synchronous `append_message` — assert this with a property test (Step 5.7).
16. **`agent-core` over `agent-gateway`** when placement is ambiguous — `agent-core` is testable without HTTP.
17. Preserve existing routing audit fields when refactoring; observability is already good. Do not regress it.
