# Dead Code & Incomplete Feature Removal Plan

> Audit date: 2026-05-19  
> Source: arch.md review + codebase grep verification

---

## 1. Dead Code — Safe to Delete

### 1.1 Unmounted UI components (`packages/ui`)

These five components are exported from `@conusai/ui` but have **zero imports** in either `apps/web` or `apps/browser-shell`. The `ui-plan.md` explicitly marks `AppShell + TabStrip + RecorderControls` for deletion.

| File | Notes |
|---|---|
| `packages/ui/src/lib/components/CommandPalette.svelte` | Never mounted in any route |
| `packages/ui/src/lib/components/TabStrip.svelte` | ui-plan.md: move to `legacy/`, then delete |
| `packages/ui/src/lib/components/RecorderControls.svelte` | ui-plan.md: move to `legacy/`, then delete |
| `packages/ui/src/lib/components/ArtifactPreview.svelte` | Never mounted; referenced only in ui-plan.md as future work |
| `packages/ui/src/lib/features/auth/LoginPanel.svelte` | Exported but no consumer imports it |

**Also remove** the corresponding barrel exports in:
- `packages/ui/src/lib/index.ts`
- `packages/ui/src/lib/features/index.ts` (`LoginPanel`)
- `packages/ui/src/lib/components/` barrel if one exists

---

### 1.2 Broken test files (`apps/web/src/tests/`)

Both files import `../lib/api/stream` which **does not exist** in the repo. The tests have never run successfully.

| File | Broken import |
|---|---|
| `apps/web/src/tests/sse-parser.test.ts` | `import { streamChat } from '../lib/api/stream'` |
| `apps/web/src/tests/reconnect.test.ts` | `import { streamChat } from '../lib/api/stream'` |

**Options (pick one):**
- **Delete** both files (SSE parsing is exercised by `createChatStream` in `@conusai/ui`).
- Repoint the import to `@conusai/sdk` once a `streamChat` export is confirmed there.

---

### 1.3 `capture_tab_screenshot` stub (`apps/browser-shell/src-tauri/src/recorder.rs`)

The command always returns `Err("capture_image requires Tauri >= 2.2; upgrade the dependency")`. It is registered in `invoke_handler` in `lib.rs` but is effectively non-functional. The file also contains a dead uncompressed PNG encoder that exists only to support this stub.

**Remove:**
- `pub async fn capture_tab_screenshot(...)` (lines ~153–167)
- The PNG encoder helper functions below it (~170+)
- The `recorder::capture_tab_screenshot` entry from `invoke_handler!` in `lib.rs`

**Restore when:** Tauri is upgraded to ≥ 2.2 and `capture_image()` API is available.

---

## 2. Incomplete Features — Fix or Remove

### 2.1 `featureFlags.svelte.ts` — exported but never instantiated

`createFeatureFlags` is exported from `@conusai/ui/stores` and re-exported from `packages/ui/src/lib/index.ts`, but **no app ever calls it**. The flags (`recorder`, `tabs`, `traceReplay`) are intended as runtime gates.

**Options:**
- **Wire it up** (arch.md backlog item 7): instantiate in `+layout.svelte` for each app and wrap `RecorderControls`, `TabStrip`, and trace replay UI in flag checks.
- **Delete** `featureFlags.svelte.ts` and remove its exports from `stores/index.ts` and `index.ts` if runtime gating is deferred.

---

### 2.2 `MobileShell.workspaceNodes` — always empty

`apps/browser-shell/src/lib/mobile/MobileShell.svelte` declares:

```ts
let workspaceNodes = $state<WorkspaceNode[]>([]);
```

This array is passed to `DrawerWorkspaceTree` but is **never populated** from the SDK. `DrawerRecentChats` shows no workspace context as a result.

**Fix:** Call `sdk.workspaces.tree()` on mount (inside `onMount`) and assign the result to `workspaceNodes`, mirroring the hydration pattern in `apps/web/src/routes/+page.server.ts`.

---

### 2.3 `ArtifactsScreen` — non-interactive artifact rows

`apps/browser-shell/src/lib/mobile/screens/ArtifactsScreen.svelte` passes a no-op handler to every row:

```svelte
onClick={() => {}}
```

The entire artifacts screen is visually present but produces no action on tap.

**Fix:** Implement the open/preview action — open the artifact in a new tab via `create_tab` (Tauri command) or display it in `ArtifactPreview` once that component is wired (see §1.1).

---

### 2.4 `CapabilitiesScreen` → chat composer — screen switches but does not dispatch

Tapping a capability in `CapabilitiesScreen` switches to the chat screen but does not prefill the composer or auto-send. This makes the capability list a read-only directory.

**Fix:** After switching screen, call a shared `chatStream.send(prefillText)` or set `composerValue` via a store/prop so the user's selected capability seeds the next message.

---

### 2.5 Login action bypasses `sessionAdapter` (`apps/web`)

`apps/web/src/routes/login/+page.server.ts` calls the low-level `sign(name, plan)` helper directly instead of `sessionAdapter.issue(...)`. This means the `BackendJwtAdapter` (activated by `BACKEND_AUTH_LOGIN_URL`) and the Zitadel OIDC adapter are never exercised by the form-based login path.

**Fix:**

```ts
// Before
cookies.set(COOKIE_NAME, sign(name, plan), { … });

// After
const token = await sessionAdapter.issue(name, plan);
cookies.set(COOKIE_NAME, token, { … });
```

Requires adding an `issue(name, plan): Promise<string>` method to the `SessionAdapter` interface (currently only `verify` is shared).

---

## 3. Priority Order

| # | Item | Effort | Risk |
|---|---|---|---|
| 1 | ~~Delete 5 unmounted UI components + barrel exports~~ ✅ | Low | None |
| 2 | ~~Delete 2 broken test files~~ ✅ | Low | None |
| 3 | ~~Remove `capture_tab_screenshot` stub + PNG helpers~~ ✅ | Low | None |
| 4 | Fix login action to use `sessionAdapter.issue()` | Medium | Auth regression — test after |
| 5 | Hydrate `MobileShell.workspaceNodes` | Medium | None |
| 6 | Implement `ArtifactsScreen` row action | Medium | None |
| 7 | Wire `featureFlags` as runtime gate or delete | Medium | None |
| 8 | Wire capability invoke into chat composer | Medium | UX regression risk low |
