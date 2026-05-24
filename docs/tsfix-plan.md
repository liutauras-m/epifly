# TypeScript / svelte-check error fix plan

**Status:** `apps/web` reports **93 errors**, `apps/browser-shell` reports **2 errors** (both before this branch's UI work). Goal: drive both to **0 errors** without changing runtime behaviour.

**Stack reality (verified in the lockfile):**
- Svelte `^5.0.0` (runtime `5.55.5`)
- SvelteKit `^2.21.0`
- TypeScript `^5.7.3`
- Vitest `^2.0.0`
- `apps/web` tsconfig: `module: NodeNext, moduleResolution: NodeNext, strict: true`
- `apps/browser-shell` tsconfig: inherits SvelteKit's defaults, `strict: true`

---

## 0. Inventory — what's actually broken

Grouped by root cause (most → least frequent):

| # | Category | Count | Root cause |
|---|---|---|---|
| A | Missing `.js` extension on relative / `$lib` imports | ~22 | `module: NodeNext` requires explicit extensions; legacy imports omit them |
| B | `Binding element implicitly has 'any' type` in server endpoints | ~22 | Handlers don't import `RequestHandler` / `PageServerLoad` from `./$types.js` |
| C | `lucide-svelte` named exports missing in `packages/ui` | ~30 | Two lucide packages installed; ui uses the deprecated `lucide-svelte@0.477` whose typings are incompatible with Svelte 5's stricter `ComponentType` checking |
| D | `Cannot find module '@tauri-apps/plugin-deep-link'` | 1 (shows in both apps) | Plugin used in `packages/ui/.../routing/initialRoute.ts` but not in any `package.json` |
| E | Browser-shell `CustomStreamFn` type mismatch | 1 | SDK's `chatApi.stream` was tightened to `Omit<StreamChatParams, 'fetch'\|'baseUrl'>`; `tauri-stream.ts` still takes the full `StreamChatParams` |
| F | `vite.config.ts` — `test` not in `UserConfigExport` | 1 | Vitest config in `vite.config.ts` imports `defineConfig` from `vite` instead of `vitest/config` |
| **Total** | | **~77** unique → fan-out to **95** instances | |

The pure-count is misleading: **four files own ~40% of the errors** (the `*.server.ts` files in `apps/web`). Fixing them is a single bulk edit.

---

## 1. Category A — missing `.js` extensions (`apps/web`)

### Symptom
```
ERROR "src/lib/server/env.ts" 1:29
  "Relative import paths need explicit file extensions in ECMAScript imports
   when '--moduleResolution' is 'node16' or 'nodenext'.
   Did you mean './session.js'?"
```

### Why it's correct that we keep `NodeNext`
The whole monorepo's downstream `packages/*` are ESM-published with explicit `.js` extensions; SvelteKit's server-side build uses Node ESM resolution. Loosening `moduleResolution` to `Bundler` would mask real-bug categories (especially when packaging with `@sveltejs/adapter-node`).

### Fix
Add `.js` to every relative / `$lib/...` import in the `apps/web` server-side and shared files:

| File | Change |
|---|---|
| `apps/web/src/hooks.server.ts:2` | `'$lib/server/session'` → `'$lib/server/session.js'` |
| `apps/web/src/lib/server/env.ts:1` | `'./session'` → `'./session.js'` |
| `apps/web/src/routes/+layout.server.ts:2-3` | `'./$types'` → `'./$types.js'`, `'$lib/server/session'` → `'$lib/server/session.js'` |
| `apps/web/src/routes/+layout.svelte:6` | `'./$types'` → `'./$types.js'` |
| `apps/web/src/routes/+page.svelte:5` | `'$lib/sdk'` → `'$lib/sdk.js'` |
| `apps/web/src/routes/+page.server.ts:1-3` | three imports |
| `apps/web/src/routes/account/+page.server.ts:2` | one import |
| `apps/web/src/routes/account/billing/+page.server.ts:2` | one import |
| `apps/web/src/routes/account/usage/+page.server.ts:2` | one import |
| `apps/web/src/routes/auth/+server.ts:6` | one import |
| `apps/web/src/routes/auth/callback/+server.ts:6` | one import |
| `apps/web/src/routes/auth/logout/+server.ts:5` | one import |
| `apps/web/src/routes/login/+page.server.ts:2-3` | two imports |
| `apps/web/src/routes/logout/+server.ts:2-3` | two imports |

**Single command to bulk-fix:** none — needs manual edits because we can't blindly add `.js` to every import (would break package specifiers). Recommend a per-file `Edit` pass; total ~20 mechanical changes, no design decisions.

### Verification
Clears ~22 errors. Spot-check by running the `dev` script — Vite uses its own resolver and will still serve, but TypeScript will go quiet.

---

## 2. Category B — untyped server endpoints

### Symptom
```
ERROR "src/routes/+page.server.ts" 17:46
  "Binding element 'locals' implicitly has an 'any' type."
```

### Why
The handlers use destructuring (`async ({ locals, fetch })`) without a function type. SvelteKit auto-generates per-route types in `.svelte-kit/types/...`; we just have to import them.

### Fix
Per the [SvelteKit type docs](https://svelte.dev/docs/kit/types), use the generated `./$types.js` types. Pattern:

```ts
// before
export const load = async ({ locals, url }) => { ... };
export const POST = async ({ request, cookies }) => { ... };

// after
import type { PageServerLoad, Actions } from './$types.js';
export const load: PageServerLoad = async ({ locals, url }) => { ... };
export const actions: Actions = { default: async ({ request, cookies }) => { ... } };
```

For `+server.ts` endpoints:

```ts
import type { RequestHandler } from './$types.js';
export const GET: RequestHandler = async ({ url, cookies }) => { ... };
```

### Files to touch
| File | Type to add |
|---|---|
| `apps/web/src/routes/+layout.server.ts` | `LayoutServerLoad` |
| `apps/web/src/routes/+page.server.ts` | `PageServerLoad` |
| `apps/web/src/routes/account/+page.server.ts` | `PageServerLoad` |
| `apps/web/src/routes/account/billing/+page.server.ts` | `PageServerLoad`, `Actions` |
| `apps/web/src/routes/account/usage/+page.server.ts` | `PageServerLoad` |
| `apps/web/src/routes/auth/+server.ts` | `RequestHandler` |
| `apps/web/src/routes/auth/callback/+server.ts` | `RequestHandler` |
| `apps/web/src/routes/auth/logout/+server.ts` | `RequestHandler` |
| `apps/web/src/routes/login/+page.server.ts` | `PageServerLoad`, `Actions` |
| `apps/web/src/routes/logout/+server.ts` | `RequestHandler` |

### Verification
Clears the ~22 "implicit any" errors in one pass per file. If the route's `Actions` fail (because the handler returns a shape SvelteKit doesn't like), that's a real bug to fix not a typing hack.

---

## 3. Category C — `lucide-svelte` icon imports broken in `packages/ui`

### Symptom
```
ERROR "packages/ui/src/lib/components/Composer.svelte" 27:12
  "Module '\"lucide-svelte\"' has no exported member 'Send'."
```

### Why
Two lucide packages are installed:
- `lucide-svelte@0.477.0` (deprecated unscoped, pinned in `packages/ui/package.json`)
- `@lucide/svelte@1.16.0` (current scoped package, used in `apps/web` directly)

Under Svelte 5's tighter type checking, the older `lucide-svelte@0.477` no longer surfaces its named exports through `svelte-check`. The same icons work fine when imported from `@lucide/svelte`.

### Fix — adopt `@lucide/svelte` everywhere

**Step 1:** In `packages/ui/package.json`:
- Remove `"lucide-svelte": "^0.477.0"` from `dependencies`
- Add `"@lucide/svelte": "^1.16.0"` to `dependencies`

**Step 2:** In every file currently importing from `lucide-svelte`, swap the specifier:

| File | Change |
|---|---|
| `packages/ui/src/lib/components/PlanBadge.svelte` | `from 'lucide-svelte'` → `from '@lucide/svelte'` |
| `packages/ui/src/lib/components/PlanCard.svelte` | same |
| `packages/ui/src/lib/components/QuotaBanner.svelte` | same |
| `packages/ui/src/lib/components/Chip.svelte` | same |
| `packages/ui/src/lib/components/EmptyState.svelte` | same |
| `packages/ui/src/lib/components/ToastHost.svelte` | same |
| `packages/ui/src/lib/components/Composer.svelte` | same |
| `packages/ui/src/lib/components/Icon.svelte` | same |
| `packages/ui/src/lib/components/Icon.fixtures.ts` | same |
| `packages/ui/src/lib/components/Button.fixtures.ts` | same |
| `packages/ui/src/lib/components/Chip.fixtures.ts` | same |
| `packages/ui/src/lib/features/CapabilityPinChip.svelte` | same |

This is a single repo-wide grep-replace:
```bash
grep -rl "from 'lucide-svelte'" packages/ui/src/ \
  | xargs perl -pi -e "s/from 'lucide-svelte'/from '\@lucide\/svelte'/g"
```

(Identical command for the double-quote variant.) Then `pnpm install`.

### Risk
The named-export surface is identical between the two packages (both follow the upstream Lucide icon list). The only behavioural diff is `Icon` instances become `Svelte 5 Component<…>` instead of `SvelteComponent` — already handled by our `Icon.svelte` wrapper which uses `Component<any>` typing.

### Verification
Clears ~30 errors. Build `pnpm --filter @conusai/ui build` to ensure tree-shaking still works.

---

## 4. Category D — `@tauri-apps/plugin-deep-link`

### Symptom
```
ERROR "packages/ui/src/lib/routing/initialRoute.ts" 46:128
  "Cannot find module '@tauri-apps/plugin-deep-link' or its corresponding type declarations."
```

### Why
The shared `initialRoute.ts` imports `@tauri-apps/plugin-deep-link` so the Tauri shell can pick up `conusai://?ws=<id>` deep links. The plugin is only meaningful inside Tauri; the web app's `apps/web` doesn't need it at runtime. But TS doesn't know that — it tries to resolve the import statically and fails.

### Fix — two options, pick one

**Option A (cleaner): install the plugin once in the workspace** so types resolve in both apps; the runtime side stays guarded by `isTauri`.

```bash
pnpm add -D @tauri-apps/plugin-deep-link --filter @conusai/ui
```

(The plugin's *runtime* requires the Tauri shell, but its types are pure TS and safe to install anywhere.)

**Option B (less footprint): move the deep-link branch into `apps/browser-shell`** and call it from a callback prop on `initialRoute`. More refactor, but stops bleeding Tauri specifics into the shared package. Recommended only if we plan to add more Tauri-only logic.

**Recommend A** — one-liner, no API breakage.

### Verification
Clears 1 error each in web and browser-shell svelte-check runs.

---

## 5. Category E — browser-shell `CustomStreamFn` type drift

### Symptom
```
ERROR "src/routes/+page.svelte" 27:45
  "Type '... StreamChatParams ...' is not assignable to type 'CustomStreamFn'.
   Property 'baseUrl' is missing in type 'Omit<StreamChatParams, "fetch" | "baseUrl">'
   but required in type 'StreamChatParams'."
```

### Why
SDK contract:
```ts
// packages/sdk/src/chatApi.ts:7
stream(params: Omit<StreamChatParams, 'fetch' | 'baseUrl'>, opts?: { reconnect?: boolean })
```

The SDK's `CustomStreamFn` (the type for a swappable transport) takes the *omitted* shape — `baseUrl` is injected by the SDK itself, not the caller. But `apps/browser-shell/src/lib/tauri-stream.ts` declares:

```ts
export async function* streamChatTauri(params: {
  message: string;
  sessionToken: string;
  // ... etc, NO baseUrl
}): AsyncGenerator<ChatStreamDelta>
```

The shape is right, but the adapter in `+page.svelte` re-types it as `StreamChatParams` (with `baseUrl`), then assigns it to `CustomStreamFn` (without `baseUrl`) → type mismatch.

### Fix
In `apps/browser-shell/src/routes/+page.svelte`, change the adapter's param type from `StreamChatParams` → the SDK's `CustomStreamFn` parameter type:

```ts
// before
const tauriStreamFn = isTauri
  ? (params: import('@conusai/sdk').StreamChatParams) =>
      streamChatTauri({ ... })
  : undefined;

// after — use the SDK's exported CustomStreamFn type directly
import type { CustomStreamFn } from '@conusai/sdk';

const tauriStreamFn: CustomStreamFn | undefined = isTauri
  ? (params) => streamChatTauri({
      message: params.message,
      sessionToken: getSessionToken() ?? '',
      threadId: params.threadId,
      workspaceNodeId: params.workspaceNodeId,
      attachmentIds: params.attachmentIds,
      forcedCapability: params.forcedCapability,
      signal: params.signal,
    })
  : undefined;
```

If `CustomStreamFn` isn't yet exported from `@conusai/sdk/index.ts`, add `export type { CustomStreamFn } from './chatApi.js'`.

### Verification
Clears the 1 browser-shell error.

---

## 6. Category F — `vite.config.ts` Vitest typing

### Symptom
```
ERROR "vite.config.ts" 31:2
  "No overload matches this call. ... 'test' does not exist in type 'UserConfigExport'."
```

### Why
`defineConfig` imported from `vite` doesn't know about Vitest's `test` field. Vitest publishes its own `defineConfig` that extends the type.

### Fix
In `apps/web/vite.config.ts`:

```ts
// before
import { defineConfig } from 'vite';

// after (one of)
import { defineConfig } from 'vitest/config';
//   OR keep vite's defineConfig and add at the top of the file:
//   /// <reference types="vitest" />
```

**Recommend `vitest/config`** — explicit, no triple-slash.

### Verification
Clears the 1 vite.config error.

---

## 7. Execution order (smallest blast radius → broadest)

1. **Category F** (1 file, 1 error) — `vite.config.ts` import swap. **Effort: 1 min.**
2. **Category D** (1 dependency add) — install `@tauri-apps/plugin-deep-link` in `packages/ui`. **Effort: 2 min.**
3. **Category E** (1 file edit + maybe 1 SDK re-export) — fix `CustomStreamFn` typing. **Effort: 5 min.**
4. **Category C** (12 files + 1 package.json + reinstall) — lucide swap. **Effort: 10 min** including a `pnpm install` and full `svelte-check` re-run.
5. **Category A** (~14 files) — `.js` extensions on imports. **Effort: 20 min** if done carefully (don't add `.js` to package specifiers).
6. **Category B** (~10 files) — typed handlers via `./$types.js`. **Effort: 30 min** because each handler also needs its destructure-shape adjusted to match the generated type.

**Total effort: ~70 minutes for a clean, no-error baseline across both apps.**

### Suggested PR slicing
- **PR 1 (chore, ~15 min):** F + D + E + C — easy wins, no behavioural change.
- **PR 2 (chore, ~50 min):** A + B — touches the most files; reviewers can confirm each handler's typing matches the route folder.

Doing it in two PRs means PR 1's green CI proves the package-level fixes work before piling on the per-route changes.

---

## 8. Verification gate

After all six categories land:

```bash
# Both apps must report 0 errors:
pnpm --filter web         exec svelte-check --tsconfig tsconfig.json | grep COMPLETED
pnpm --filter browser-shell exec svelte-check --tsconfig tsconfig.json | grep COMPLETED

# Smoke-test that nothing got runtime-broken:
pnpm --filter @conusai/ui exec vitest run          # 107/108 still pass (1 unrelated token-parity test is pre-existing)
pnpm --filter web         build                    # SvelteKit build
pnpm --filter browser-shell build                  # Tauri webview bundle

# Visual smoke (your skill):
# web on :5173, browser-shell on :5174 — both render ShellScreen/ShellLoginScreen identically to today.
```

### Acceptance criteria
- `0 ERRORS` from `svelte-check` on both apps.
- `pnpm --filter @conusai/ui exec vitest run`: exactly the 107 passes we have today (the 1 token-parity failure is unrelated and tracked separately).
- No new warnings beyond today's 14 (web) / 9 (browser-shell).
- No diff in rendered DOM at `/`, `/login`, `/account`, `/account/billing`, `/account/usage` for the web app.

---

## 9. What's explicitly out of scope

| Item | Why deferred |
|---|---|
| Migrating to `module: Bundler` resolution | Would mask real ESM packaging bugs once we ship via `adapter-node`. The `.js` extension cost is genuine ESM discipline, not noise. |
| Stricter typing on `chatStream: any` in `ShellPage` / `ShellScreen` | The `createChatStream` return type isn't exported publicly; broad typing is intentional. Separate task to surface a public `ChatStream` type from `packages/ui/features`. |
| Pre-existing `tokens.css` ↔ `tokens.json` parity test failure | One test, not a TypeScript error, and orthogonal — fix in its own PR. |
| Pre-existing 14 svelte-check warnings (`a11y_*`, `Drawer.svelte` RTL unused selectors, etc.) | Warnings, not errors. Plan separately if we want a "0 warnings" bar. |

---

## 10. Risk register

| Risk | Mitigation |
|---|---|
| `@lucide/svelte` v1 exports differ from `lucide-svelte` v0.477 for some icon name | Spot-check after migration; the upstream Lucide icon names haven't changed in years, but if any are renamed the error will be `has no exported member 'X'` again — fix point-by-point. |
| Adding `.js` to a `$lib/...` path that resolves to a `.svelte` file | `$lib/...` is a SvelteKit alias; Svelte files keep their `.svelte` extension in imports. Only `.ts` → `.js` substitution applies. |
| `./$types.js` regenerates differently after `svelte-kit sync` | Run `pnpm --filter web exec svelte-kit sync` once before typing each handler so generated types reflect the current routes. |
| Installing `@tauri-apps/plugin-deep-link` in `packages/ui` increases install graph for `apps/web` | Pure-TS plugin; types-only at install time. Negligible weight. |
