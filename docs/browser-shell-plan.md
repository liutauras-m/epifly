# Browser Shell ↔ ConusAI Web Parity Plan

**Status:** merge-ready · 2026-05-10 (final, post-Rust-team-review)
**Owners:** platform / frontend / Rust (Phase 5–6)
**Related:** [docs/tasks/app.md](tasks/app.md), [docs/adr/006-tauri-browser-shell.md](adr/006-tauri-browser-shell.md), [docs/adr/008-multi-platform-shell.md](adr/008-multi-platform-shell.md)

> **Decision (final):** Approved. Option B (shared `@conusai/ui` + dual idiomatic SvelteKit apps) is the 2026 gold-standard for a multi-platform, capability-based agent platform. Refinements landed in this revision:
>
> 1. **`createCapabilityRendererRegistry()`** (Phase 4) — pure `.ts` module + thin runes context provider (`.svelte.ts`). Frontend mirror of the Rust `CapabilityRegistry` / `SemanticCapabilityRouter`. Reactive registration, optional `fallbackRenderer`, no global state, zero-Svelte consumers possible.
> 2. **OpenAPI → TS generation pulled forward into Phase 3** — keeps `@conusai/sdk` 100% in sync with `agent-gateway` utoipa routes; makes `CapabilityCard` 1:1 with Rust `agent-core::capability::CapabilityCard`. CI asserts structural hash parity.
> 3. **`device_auth.rs` + `DeviceAuthService` + `DeviceTokenProvider` + `DeviceAuthAdmin` traits** (Phase 5) — rename from `keychain.rs`; thin service wrapping `Arc<RwLock<Stronghold>>` with explicit `init_stronghold` command, mock-Stronghold unit tests, and a separate admin trait reserved for token rotation / audit.
> 4. **`TabManager` service** (Phase 6) — extract `set_active_tab_content_bounds` + future multi-window logic into [tabs.rs](../apps/browser-shell/src-tauri/src/tabs.rs); `main.rs` stays a registration shell.
> 5. **Shared `ThemeScript`** + **assets manifest** + **`ConusSdk` type alias** — micro-SRP polish that eliminates the last copy-paste between apps.
>
> **No `CapabilityRegistry` in the Tauri backend.** Recorder, tabs, and screenshot are *shell chrome*, not LLM tools. The `ArtifactBridge` → workspace node → `TraceReplayCapability` renderer flow already closes the loop.
>
> Multi-platform consequence (see §14): macOS + Windows desktop are 100% single-source after Phase 6; iOS via Tauri Mobile is a trivial follow-up (~8–10 AI-hours) that reuses the exact same `@conusai/ui` + `@conusai/sdk` packages.

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

**Backend boundary (explicit):** the Tauri shell does **not** host a Rust `CapabilityRegistry`. Recorder, tabs, screenshot, and Stronghold are *shell chrome / native services*, not LLM-callable tools. The only canonical capability surface lives in `agent-core` and is consumed by the frontend `createCapabilityRendererRegistry()` via OpenAPI-generated `CapabilityCard` types. This keeps SRP clean: backend = capability provider, shell = chrome + transport, UI = renderer.

## 4. Cross-cutting standards (apply in every phase)

These are non-negotiable 2026 conventions; new code that violates them is rejected at PR review.

### 4.1 Canonical package layout

`@conusai/ui` enforces this exact tree (matches the dominant 2026 Svelte 5 design-system convention and mirrors the Rust `agent-core` module split):

```
packages/ui/
├── src/
│   └── lib/
│       ├── components/        ← presentational primitives (ThemeSwitcher, ToastHost, …)
│       ├── features/          ← feature slices (AgentChatStream, WorkspaceExplorer, …)
│       ├── capabilities/      ← CapabilityRendererRegistry + per-capability cards (mirrors Rust CapabilityRegistry)
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
6. **Assets manifest.** Generate `packages/ui/dist/assets-manifest.json` at build time enumerating every file under `assets/`. Both apps run a CI step `pnpm assets:verify` that fails the build if a referenced asset is missing from the manifest — eliminates the "missing favicon in prod bundle" class of bugs.

**Verify:** `pnpm --filter web build && pnpm --filter browser-shell build`; `pnpm assets:verify` green; visually confirm favicon and sidebar logo render in both `vite preview` and `pnpm tauri dev`.

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
5. In `apps/browser-shell/src/app.html`, add the `data-theme` attribute and the flash-prevention `<script>` from [apps/web/src/app.html](../apps/web/src/app.html#L8-L17). The script body lives **once** in `packages/ui/src/lib/components/ThemeScript.ts` (exported as a string constant) and is injected by both apps via SvelteKit's `%sveltekit.head%` mechanism — no copy-paste.
6. Promote the theme toggle (currently inlined in `apps/web/+page.svelte`) into `packages/ui/src/lib/components/ThemeSwitcher.svelte`. Back it with `packages/ui/src/lib/stores/themeStore.ts` — a runes-based store with two adapters selected at app boundary:
   - web: `localStorage`
   - shell: `tauri-plugin-store`
7. Add a tiny `ThemeProvider.svelte` (runes) that wraps each app's `+layout.svelte` and emits a Tauri event `theme-change` so Rust can re-tint the macOS tray icon when needed.
8. Both apps' root `+layout.svelte` ends up with: `import '@conusai/ui/foundry.css'; <ThemeProvider>{@render children()}</ThemeProvider>`.

**Verify:**
- `apps/web`: paper ↔ forge toggle still works, no FOUC.
- `apps/browser-shell`: launch in `pnpm tauri dev`, confirm Fraunces renders offline (DevTools Network → fonts come from `app://`); theme toggle works; tray icon recolours on theme change.

## 7. Phase 3 — Shared API client & types

**Goal:** one TypeScript SDK for all REST/SSE calls, kept in lock-step with `agent-gateway` utoipa routes. Web and shell pick different transports but the same surface.
**Effort:** ~3.5 AI-hours.

1. Audit [apps/web/src/lib/api/](../apps/web/src/lib/api): `client.ts`, `endpoints.ts`, `glyphs.ts`, `stream.ts`, `types.ts`, `workspaces.ts`. Pure ESM; only SvelteKit dep is the `fetch` argument.
2. Move all of `lib/api/` into `packages/sdk/src/`.
3. Replace the `fetch` parameter with a constructor-injected fetch implementation, exposing a single named factory `createConusSdk` plus a public `ConusSdk` type alias (matches `rig-core`'s `Agent` + `AgentBuilder` ergonomics; trivial DI in tests):
   ```ts
   export type ConusSdk = ReturnType<typeof createConusSdk>;

   export function createConusSdk(opts: ClientOpts) {
     const client = createInternalClient(opts);
     return {
       workspaces:   workspacesApi(client),
       chat:         chatApi(client),
       threads:      threadsApi(client),
       files:        filesApi(client),
       capabilities: capabilitiesApi(client),
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
6. Move shared response shapes from [apps/web/src/lib/api/types.ts](../apps/web/src/lib/api/types.ts) into `@conusai/types`. **Pull the OpenAPI → TS generation forward into this phase**: harden [scripts/openapi-to-types.sh](../scripts/openapi-to-types.sh), wire it as a `prebuild` step in `packages/types/package.json`, and add it to the `turbo build` pipeline so every build (and CI run) regenerates against `agent-gateway`'s utoipa output. Add a CI step `pnpm types:assert-parity` that hashes the JSON-Schema for `CapabilityCard` (and other shared types) on both Rust and TS sides and fails on mismatch — cheap structural-diff guard against silent drift. The regenerated `CapabilityCard` is now a **1:1 mirror** of the Rust `agent-core::capability::CapabilityCard`, which the Phase 4 `CapabilityRendererRegistry` consumes directly.
7. Re-export from `@conusai/sdk`:
   ```diff
   - import { workspacesApi } from '$lib/api';
   + import { createConusSdk } from '@conusai/sdk';
   ```
8. Preserve the existing discriminated `{ data } | { error }` return union — no `throw` in SDK methods, no `any`.

**Verify:** move [tests/sse-parser.test.ts](../apps/web/src/tests/sse-parser.test.ts), [tests/reconnect.test.ts](../apps/web/src/tests/reconnect.test.ts) into `packages/sdk/tests/`; `pnpm --filter @conusai/sdk test` green; both apps build; web smoke passes.

## 8. Phase 4 — Shared chat + workspace features (+ capability registry)

**Goal:** the 968-line [apps/web/src/routes/+page.svelte](../apps/web/src/routes/+page.svelte) becomes a thin route (≤ 80 LoC) that composes shared, runes-only feature slices, and tool/capability rendering is dispatched through a frontend mirror of the Rust `CapabilityRegistry`.
**Effort:** ~13 AI-hours (largest chunk; split across 2–3 PRs).

Decompose into `packages/ui/src/lib/features/` and `packages/ui/src/lib/capabilities/`:

| Component / factory | Responsibility | Source LoC |
|---|---|---|
| `AgentChatComposer.svelte` | textarea, attachments, focus, autogrow; queries `/v1/capabilities/search` for inline suggestions | ~80 |
| `AgentChatStream.svelte` | message list, word-token animation, scroll-near; renders tool/capability outputs via the registry (no per-capability `if`s) | ~180 |
| `ToolCallCard.svelte` | thin dispatcher → `getRendererForCapability(card)`; renders fallback for unregistered capabilities | ~60 |
| `WorkspaceExplorer.svelte` | tree + lazy load + search + recents | ~250 |
| `NewNodeDialog.svelte`, `ConfirmDialog.svelte`, `MoveDialog.svelte`, `ShareDialog.svelte` | promote from [lib/workspace/dialogs/](../apps/web/src/lib/workspace/dialogs) as-is | — |
| `createChatStream()` (rune factory) | wraps `conusSdk.chat.stream`, AbortController, 45 s inactivity timer, tool-card map | ~120 |
| `createWorkspaceTree()` (rune factory) | tree state, lazy loading, search debounce, refresh | ~80 |
| `CapabilityRendererRegistry.ts` | frontend mirror of Rust `CapabilityRegistry`; maps capability name → Svelte component | ~50 |

### 8.1 `createCapabilityRendererRegistry()` (new micro-slice)

Two files, strict SRP:

- `packages/ui/src/lib/capabilities/CapabilityRendererRegistry.ts` — **pure module**. No Svelte imports. Plain runes-friendly `Map` + `register` / `unregister` / `get` / `names`. Consumable by Vitest, Node scripts, and any future non-Svelte shell.
- `packages/ui/src/lib/capabilities/CapabilityRendererRegistry.svelte.ts` — **thin context provider** (~12 LoC). Wraps the pure module in `setContext` / `getContext` so Svelte components consume it reactively.

Mirrors the Rust `CapabilityRegistry` lookup pattern exactly; gives reactivity for runtime-loaded capabilities (e.g. native shell capabilities registered post-boot) at zero cost for the static case.

```ts
// CapabilityRendererRegistry.ts (pure)
import type { Component } from 'svelte';
import type { CapabilityCard } from '@conusai/types';

type Renderer = Component<{ card: CapabilityCard }>;

export interface CapabilityRendererRegistry {
  register(name: string, renderer: Renderer): void;
  unregister(name: string): void;
  get(card: CapabilityCard): Renderer | null;
  readonly names: readonly string[];
}

export interface CreateRegistryOpts {
  fallbackRenderer?: Renderer;
}

export function createCapabilityRendererRegistry(opts: CreateRegistryOpts = {}): CapabilityRendererRegistry {
  const renderers = $state(new Map<string, Renderer>());
  return {
    register(name, renderer) { renderers.set(name, renderer); },
    unregister(name)         { renderers.delete(name); },
    get(card)                { return renderers.get(card.name) ?? opts.fallbackRenderer ?? null; },
    get names()              { return Array.from(renderers.keys()); }
  };
}
```

```ts
// CapabilityRendererRegistry.svelte.ts (thin provider)
import { getContext, setContext } from 'svelte';
import {
  createCapabilityRendererRegistry,
  type CapabilityRendererRegistry,
  type CreateRegistryOpts
} from './CapabilityRendererRegistry';

const KEY = Symbol('conusai.capability-registry');

export function provideCapabilityRendererRegistry(opts?: CreateRegistryOpts): CapabilityRendererRegistry {
  const r = createCapabilityRendererRegistry(opts);
  setContext(KEY, r);
  return r;
}

export function useCapabilityRendererRegistry(): CapabilityRendererRegistry {
  const r = getContext<CapabilityRendererRegistry>(KEY);
  if (!r) throw new Error('provideCapabilityRendererRegistry() not called in a parent layout');
  return r;
}
```

Usage in each app's root `+layout.svelte`:

```svelte
<script lang="ts">
  import { provideCapabilityRendererRegistry } from '@conusai/ui/capabilities';
  import DefaultToolCallRenderer from '$lib/DefaultToolCallRenderer.svelte';

  const registry = provideCapabilityRendererRegistry({ fallbackRenderer: DefaultToolCallRenderer });
  // shell-only: registry.register('TraceReplayCapability', TraceReplayRenderer);
</script>
```

Why this matters:
- **Rust ↔ Frontend symmetry.** Backend already has `CapabilityRegistry` + `SemanticCapabilityRouter` + `CapabilityCard`. This is the canonical client-side counterpart, with the same `register` / `get` / `names` surface.
- **SRP.** Pure logic is Svelte-free and unit-testable in isolation; the provider is a 12-line context shim.
- **Open/closed principle.** New capabilities (`DynamicPromptCapability`, `TraceReplayCapability`, future `BrowserAutomationCapability`, etc.) ship a renderer alongside their Rust implementation; nothing in `AgentChatStream` or `ToolCallCard` changes.
- **Reactivity for free.** `$state`-backed map means components re-render when shell registers new renderers at runtime (e.g. on Stronghold unlock).
- **No global state.** Each app/test/story owns its own registry — no leakage between Vitest cases.
- **Semantic suggestions in the composer.** `AgentChatComposer` calls `conusSdk.capabilities.search(query)` (already exposed by the gateway) and lists matches as inline affordances — directly surfaces the `SemanticCapabilityRouter` ranking.
- **Recorder loop closure.** Recorder traces upload via `ArtifactBridge` → workspace node → on click, `ArtifactPreview` delegates to the registered renderer for `TraceReplayCapability`. **No `CapabilityRegistry` exists on the Rust shell side** — recorder/tabs are shell chrome, not LLM tools.
- **Renderer contract:** registered renderers may only consume `@conusai/ui` primitives + `ConusSdk` calls — no platform-specific imports. Documented in `packages/ui/src/lib/capabilities/README.md`.

### 8.2 Steps

1. Land each component as a pure copy-paste from `apps/web` into `packages/ui`, **converting class-style reactivity to runes** as the only behaviour change. Run `apps/web` after each move; nothing else should regress.
2. Move `lib/ui/toast.svelte.ts`, `lib/ui/actions.ts`, `lib/ui/LiveAnnouncer.svelte` into `@conusai/ui` (`stores/`, `utils/`, `components/` respectively).
3. Move `lib/workspace/context.svelte.ts` into `packages/ui/src/lib/stores/`.
4. Add `packages/ui/src/lib/stores/featureFlags.ts` (runes) so the shell can disable/enable slices (e.g. recorder UI affordances) without forking components.
5. Add `CapabilityRendererRegistry` and refactor `ToolCallCard` to dispatch through it. Register a default renderer for the existing tool-call shape so behaviour is unchanged.
6. Reduce `apps/web/src/routes/+page.svelte` to ≤ 80 LoC composing `WorkspaceExplorer`, `AgentChatComposer`, `AgentChatStream`.
7. **A11y guardrails** enforced while extracting:
   - All dialogs use `<dialog>` with `inert` siblings + focus-trap.
   - Streaming text uses `aria-live="polite"` via `LiveAnnouncer`.
   - Composer is a `<form>` with explicit submit; Enter sends, Shift+Enter newlines.
   - Tool cards expose status via `aria-label` on the badge, not colour alone.

**Verify:**
- Web: existing Playwright [e2e/smoke.test.ts](../apps/web/e2e/smoke.test.ts) still passes.
- New: `packages/ui/tests/` with vitest + `@testing-library/svelte` covering `AgentChatStream` word-flush, `WorkspaceExplorer` lazy load, `ToolCallCard` registry dispatch + status transitions, `CapabilityRendererRegistry` lookup + fallback, and **a canary test asserting that a renderer dynamically registered after mount is immediately picked up by `ToolCallCard`** (proves runtime symmetry with Rust `CapabilityRegistry`).
- 100% rune usage in promoted components (lint rule).

## 9. Phase 5 — Shell auth (Stronghold-native)

**Goal:** the shell does not need SvelteKit `hooks.server.ts` or cookie session; it authenticates via a device token loaded from Stronghold (already partially scaffolded in [apps/browser-shell/src/routes/+layout.svelte](../apps/browser-shell/src/routes/+layout.svelte#L36-L52)).
**Effort:** ~3 AI-hours (≈1 Rust + 2 TS). Rust owner ships the module + commands in a single PR.

1. Define a `TokenProvider` interface in `@conusai/sdk`:
   ```ts
   export interface TokenProvider {
     get(): Promise<string | null>;
     set(token: string): Promise<void>;
     clear(): Promise<void>;
   }
   ```
2. **Web** implementation: cookie-backed (`event.locals.session` server-side, `/v1/session` client-side). No change to existing flow.
3. **Shell** implementation: rename `apps/browser-shell/src-tauri/src/keychain.rs` → `device_auth.rs` (the public surface is *device auth*; "keychain" is an implementation detail of one platform adapter). Introduce a thin `DeviceAuthService` wrapping `Arc<RwLock<Stronghold>>` that implements a `DeviceTokenProvider` trait — symmetric with the TS `TokenProvider` interface and ready for iOS Secure Enclave / Android Keystore adapters with zero changes to the Tauri commands.

   ```rust
   // apps/browser-shell/src-tauri/src/device_auth.rs
   use iota_stronghold::{Stronghold, ClientError};
   use std::sync::Arc;
   use tauri::{State, Manager};
   use tokio::sync::RwLock;
   use thiserror::Error;

   #[derive(Debug, Error, serde::Serialize)]
   #[serde(tag = "type", content = "message")]
   pub enum DeviceAuthError {
       #[error("vault not provisioned")]
       NotProvisioned,
       #[error("device token missing")]
       TokenMissing,
       #[error(transparent)]
       Stronghold(#[from] ClientError),
       #[error(transparent)]
       Io(#[from] std::io::Error),
   }

   #[async_trait::async_trait]
   pub trait DeviceTokenProvider: Send + Sync {
       async fn get(&self)   -> Result<String, DeviceAuthError>;
       async fn set(&self, token: String) -> Result<(), DeviceAuthError>;
       async fn clear(&self) -> Result<(), DeviceAuthError>;
   }

   /// Reserved for super-admin operations (token rotation, audit). Kept separate so the
   /// hot path (`DeviceTokenProvider`) stays minimal and the admin surface can evolve
   /// without touching command signatures.
   #[async_trait::async_trait]
   pub trait DeviceAuthAdmin: DeviceTokenProvider {
       async fn rotate(&self) -> Result<(), DeviceAuthError>;
       async fn audit(&self)  -> Result<Vec<AuditEntry>, DeviceAuthError>;
   }

   pub type StrongholdState = Arc<RwLock<Stronghold>>;

   pub struct DeviceAuthService(StrongholdState);

   impl DeviceAuthService {
       pub async fn new(app: &tauri::AppHandle) -> Result<Self, DeviceAuthError> {
           let path = app.path().app_data_dir()?.join("device.vault");
           let stronghold = Stronghold::load_or_create(path /*, passphrase from OS keychain via plugin */)
               .await?;
           Ok(Self(Arc::new(RwLock::new(stronghold))))
       }
   }

   #[async_trait::async_trait]
   impl DeviceTokenProvider for DeviceAuthService { /* read/write/clear via Stronghold store */ }

   #[tauri::command]
   pub async fn init_stronghold(
       app: tauri::AppHandle,
       state: State<'_, DeviceAuthService>,
   ) -> Result<(), DeviceAuthError> { /* one-time vault provisioning on first launch */ Ok(()) }

   #[tauri::command]
   pub async fn get_device_token(state: State<'_, DeviceAuthService>) -> Result<String, DeviceAuthError> {
       state.get().await
   }

   #[tauri::command]
   pub async fn set_device_token(token: String, state: State<'_, DeviceAuthService>) -> Result<(), DeviceAuthError> {
       state.set(token).await
   }

   #[tauri::command]
   pub async fn clear_device_token(state: State<'_, DeviceAuthService>) -> Result<(), DeviceAuthError> {
       state.clear().await
   }
   ```

   Registration in `main.rs`:

   ```rust
   tauri::Builder::default()
       .setup(|app| {
           let svc = tauri::async_runtime::block_on(DeviceAuthService::new(app.handle()))?;
           app.manage(svc);
           Ok(())
       })
       .invoke_handler(tauri::generate_handler![
           device_auth::init_stronghold,
           device_auth::get_device_token,
           device_auth::set_device_token,
           device_auth::clear_device_token,
           // … existing recorder / tabs / telemetry commands
       ])
   ```

   - All errors are `thiserror` + `serde::Serialize` (never `anyhow::Error` — that breaks Tauri TS type generation).
   - All inputs validated via serde at the command boundary.
   - `init_stronghold` is required by the `tauri-plugin-stronghold` 2026 API and keeps Stronghold provisioning out of the Svelte layer.
   - **Mock-Stronghold tests** (`#[cfg(test)] mod tests { struct InMemoryVault; impl DeviceTokenProvider for InMemoryVault … }`) cover all four commands without a real vault. Non-negotiable production hygiene.
4. On first launch, if `get_device_token` returns `TokenMissing`, render `LoginPanel.svelte` (promoted from [apps/web/src/routes/login/+page.svelte](../apps/web/src/routes/login/+page.svelte) into `@conusai/ui/features/auth/`) with a pluggable `onSubmit(creds) => Promise<token>`.
5. After successful sign-in, call `set_device_token` (shell) or rely on cookie set by gateway (web).
6. Sign-out calls `clear_device_token` + re-mounts `LoginPanel`. Web sign-out hits `/logout`.
7. Replace the env-var token fallback (currently end of `loadTokenFromStronghold`) with an explicit "Sign in" CTA — no silent fallback in production builds.

**Security checks (OWASP-aligned):**
- Stronghold passphrase derived from OS keychain (macOS Keychain / Windows Credential Manager / GNOME Keyring) via [tauri-plugin-stronghold](https://v2.tauri.app/plugin/stronghold/) — never hardcoded.
- Token never logged.
- Shell CSP in [tauri.conf.json](../apps/browser-shell/src-tauri/tauri.conf.json) tightened to: `default-src 'self'; connect-src 'self' https://api.<gateway> wss://api.<gateway>; img-src 'self' data: blob:`. Sourced from a single `packages/config/csp.ts` so web and shell stay in sync.
- `withGlobalTauri: false` (already set).

**Verify:** manual login → quit app → relaunch → no re-auth prompt; manual sign-out → relaunch → login screen shown; Tauri-generated TS types include `DeviceAuthError` variants (`NotProvisioned`, `TokenMissing`, `Stronghold`, `Io`).

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
2. **Tab content via Tauri v2.2+ `webview_window` API** — *not* DOM coordinate hacks. Extract a dedicated `TabManager` service in [tabs.rs](../apps/browser-shell/src-tauri/src/tabs.rs) (SRP) so `main.rs` stays a registration shell and future multi-window / split-view features are trivial:
   - `TabManager` owns the `WebviewWindow` handles + active-tab id + bounds cache.
   - Exposes Tauri commands `set_active_tab_content_bounds(rect: Rect)`, `open_tab`, `close_tab`, `focus_tab` — all returning `Result<_, TabError>` (`thiserror`).
   - `TabStrip.svelte` calls `set_active_tab_content_bounds` on `mode` change and on `ResizeObserver` events from the tab-content host element.
   - Removes all manual DOM math; no `<div data-tauri-tab>` stub needed.
3. Recorder: keep [recorder.rs](../apps/browser-shell/src-tauri/src/recorder.rs) untouched. The existing `ArtifactBridge` upload returns a workspace node id; the shell dispatches a `selectNode` event that `WorkspaceExplorer` and `AgentChatStream` consume. The trace artifact is rendered by the `TraceReplayCapability` renderer registered in §8.1 — closing the recorder ↔ capability loop with zero shell-specific UI code.
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

| Phase | Owner | PRs | AI-hours | Exit criterion |
|---|---|---|---|---|
| P1 assets | frontend | 1 | 1 | Both apps render correct logo + favicon from shared package; `assets:verify` green |
| P2 design system | frontend | 1 | 2 | `app.css` deleted from `apps/web`; both apps theme-switch via shared `ThemeScript`; offline fonts in shell |
| P3 SDK + OpenAPI gen | all | 1 | 3.5 | `apps/web/src/lib/api` deleted; `createConusSdk` + `ConusSdk` alias used in both apps; `types:assert-parity` green; SDK tests green |
| P4 chat + workspace + registry | all (Rust pairs on registry symmetry) | 2–3 | 13.5 | `apps/web/+page.svelte` ≤ 80 LoC; 100% rune usage; pure registry + provider split; runtime-registration canary test green |
| P5 shell auth | Rust | 1 | 3.2 (~1.7 Rust) | `device_auth.rs` + `DeviceAuthService` + `DeviceAuthAdmin` shipped; mock-Stronghold tests green; typed `DeviceAuthError` in generated TS |
| P6 shell integration | Rust | 1 | 4 | `TabManager` service live; unified Playwright suite green; manual checklist signed off |
| **Total** | — | **6** | **27.2** | Desktop parity shipped |

Approximate token cost for LLM-assisted refactors: **~205k input / 52k output**, dominated by Phase 4 rune conversion + registry symmetry work.

### 12.1 Post-parity follow-ups (immediate, in order)

1. Bump `@conusai/ui` and `@conusai/sdk` to `0.6.0` (semver — public re-export surface changed).
2. Promote OpenAPI-generated TS client as the *only* SDK transport surface (deprecate any hand-written request shapes still lingering in `packages/types`).
3. Update [docs/adr/006-tauri-browser-shell.md](adr/006-tauri-browser-shell.md) and [docs/adr/008-multi-platform-shell.md](adr/008-multi-platform-shell.md) with: "Parity achieved via Option B + `CapabilityRendererRegistry` symmetry (2026-05-10)."
4. Run the full [docs/verify/verify.md](verify/verify.md) checklist on both `web` and `browser-shell` targets before tagging `v0.3.3`.
5. Land `apps/mobile-shell` (Tauri Mobile, ~8–10 AI-hours) — see §14.

## 13. Out of scope (intentionally deferred)

- Mobile shell ([adr/008-multi-platform-shell.md](adr/008-multi-platform-shell.md)) — see §14 for the trivial follow-up path.
- Replacing the Askama Foundry UI ([docs/tasks/app.md](tasks/app.md)) — kept as zero-JS fallback / admin surface.
- Rewriting capability cards / artifact preview beyond what already exists in `@conusai/ui`.
- New state libraries, i18n, virtualisation. Re-evaluate only after parity ships.

## 14. Multi-platform single-source guarantee

The plan is engineered around Tauri v2's canonical 2026 multi-platform story: **one Rust agent core + one Svelte UI package + Tauri as the universal runtime**. After the six phases ship, this is what we have:

| Platform | Single codebase? | How it works | Notes |
|---|---|---|---|
| **macOS** (desktop) | ✅ | `apps/browser-shell` (Tauri v2) + `@conusai/ui` + `@conusai/sdk` | Native window, Stronghold keychain, tray icon, recorder, tabs |
| **Windows** (desktop) | ✅ | Same Tauri binary, different `--target` | Same Rust core ([device_auth.rs](../apps/browser-shell/src-tauri/src/device_auth.rs), [tabs.rs](../apps/browser-shell/src-tauri/src/tabs.rs), [recorder.rs](../apps/browser-shell/src-tauri/src/recorder.rs), [telemetry.rs](../apps/browser-shell/src-tauri/src/telemetry.rs)) |
| **iOS** | Prepared (zero rework) | Future `apps/mobile-shell` (Tauri Mobile) re-imports the same `@conusai/ui` + `@conusai/sdk` | iOS Keychain adapter swaps for Stronghold via `TokenProvider` |
| **Android** | Prepared (zero rework) | Same Tauri Mobile target | Android Keystore adapter via `TokenProvider` |

Desktop parity (macOS + Windows) is **100% single-source** the moment Phase 6 lands — `tauri build --target aarch64-apple-darwin` and `--target x86_64-pc-windows-msvc` produce signed binaries from the same source tree. No conditional code in `@conusai/ui`; no per-platform forks of the chat/workspace/capability surfaces.

### 14.1 Follow-up: `apps/mobile-shell`

**Effort:** ~8–10 AI-hours (~60k input / 15k output tokens). Lands as a separate plan after desktop parity.

1. New workspace `apps/mobile-shell` with Tauri Mobile config (iOS first, Android same scaffold).
2. Imports `@conusai/ui` and `@conusai/sdk` unchanged.
3. Two platform adapters injected at app boundary (already typed in the SDK):
   - `TokenProvider` → iOS Secure Enclave / Android Keystore via `tauri-plugin-keychain`.
   - `themeStore` adapter → mobile system appearance API.
4. Add `mobile-shell` to the unified Playwright pipeline (one extra project).
5. `CapabilityRendererRegistry` works as-is; no UI rewrites.

### 14.2 What we explicitly do *not* do

- No per-platform UI forks. If a platform needs different chrome (e.g. mobile bottom tabs), it lives behind a `featureFlags` gate inside the shared component, never in a duplicated component.
- No second runtime. Electron, Capacitor, React Native are explicitly rejected — Tauri v2 + Svelte 5 covers all four targets with one stack.
- No premature mobile work. The architecture is *ready* for iOS/Android; the actual mobile build is gated behind shipping desktop parity first.
