# shadcn-svelte Migration Plan

_Drafted 2026-05-24. Owner: frontend platform. Target completion: ~6 sprints._

This document is the **detailed, phased plan to replace ConusAI's bespoke
`packages/ui` primitives + feature components with [shadcn-svelte](https://www.shadcn-svelte.com/docs/components)**
across every surface: `apps/web` (browser SSR), `apps/browser-shell` desktop
(macOS, Windows, Linux via Tauri 2) and `apps/browser-shell` mobile (iOS,
Android via Tauri 2). It is the only document for this migration — implementation
tickets reference section numbers here.

Read [docs/frontend.md](frontend.md) first for the existing architecture.

---

## 0. TL;DR

- Adopt shadcn-svelte's CLI + Bits UI primitives as the **single low-level UI
  layer**. Drop the hand-rolled primitives in `packages/ui/src/lib/components/`.
- Keep `packages/ui` as the shared package — it now houses (a) shadcn-svelte
  components copied in via the CLI, (b) our own composite "feature" + "screen"
  components built **on top of** those, (c) the existing token / motion / store /
  i18n / capability / live-resource layers.
- Bridge our Foundry design tokens to shadcn's CSS variables one direction
  only: shadcn vars `var(--*)` resolve to Foundry tokens. `tokens.json` stays
  the source of truth.
- Migrate web first, then desktop, then iOS/Android. Same Svelte source for
  every platform — only the runtime glue differs (as today).
- Strict, mechanical guard: after each phase a CI lint forbids re-introducing
  the deleted bespoke primitive.

Non-goals:

- No redesign. Visual diff must be neutral (verified by Playwright visual
  regression).
- No new features. Only swap.
- No backend or SDK changes.
- No new component libraries beyond shadcn-svelte + Bits UI + `lucide-svelte`.

---

## 1. Scope & constraints

### 1.1 In scope

| Area                          | Today                                    | After migration                                  |
| ----------------------------- | ---------------------------------------- | ------------------------------------------------ |
| Primitive components           | `packages/ui/src/lib/components/`        | shadcn-svelte under `packages/ui/src/lib/components/ui/` |
| Web shadcn wrappers            | `apps/web/src/lib/components/ui/`        | **Deleted** — re-import from `@conusai/ui`       |
| Feature components             | `packages/ui/src/lib/features/`          | Same path, rewritten to use shadcn primitives    |
| Screens                        | `packages/ui/src/lib/features/screens/`  | Same path, rewritten                             |
| Shell composition              | `ShellPage`, `ShellScreen`, `ShellLoginScreen` | Same exports, internals use shadcn `Sidebar.*`, `Sheet.*`, etc. |
| Theme tokens                   | `packages/ui/tokens/tokens.json`         | Same — extended with shadcn variable mappings    |
| Motion / stores / live / i18n  | unchanged                                | unchanged                                        |

### 1.2 Out of scope

- Backend (`apps/backend`), SDK (`packages/sdk`), types (`packages/types`).
- The Foundry token JSON values themselves (only the mapping layer changes).
- The Rust side of `browser-shell` (`src-tauri/`).
- Auth flows, OIDC, session handling.
- The capability renderer registry, live resource SWR, chat stream factory.

### 1.3 Hard constraints

1. **Svelte 5 runes only.** shadcn-svelte's latest port is runes-native, so
   this is naturally satisfied. No `svelte/store` usage may be reintroduced.
2. **Cross-platform parity.** Every primitive must render in (a) SvelteKit SSR
   (`apps/web`), (b) static SvelteKit (`apps/browser-shell` desktop WebKit /
   Edge WebView2 / WebKitGTK), and (c) static SvelteKit on iOS WKWebView and
   Android WebView. Components that require browser-only APIs must guard via
   `onMount`.
3. **No app-app imports.** The existing
   [scripts/check-cross-app-imports.mjs](../scripts/check-cross-app-imports.mjs)
   guard stays. Apps consume `@conusai/ui` only.
4. **Tokens.** The single source of truth stays
   [packages/ui/tokens/tokens.json](../packages/ui/tokens/tokens.json). All
   shadcn CSS variables must resolve back into Foundry tokens — not the other
   way round.
5. **iOS WKWebView SSE workaround.** Untouched. Only the chat **UI** changes;
   the `streamChatTauri` path remains.
6. **No visual regressions.** Each phase ships behind a feature flag
   (`featureFlagsStore.useShadcn`) and the cutover is gated on Playwright
   visual diffs passing across light + dark themes.
7. **Accessibility ≥ today's level.** axe-core scores must not regress.
   shadcn-svelte components are built on Bits UI, which is WAI-ARIA compliant —
   this should be a net improvement.
8. **Motion budget.** The existing `motion:durations` + `motion:purpose` lints
   continue to gate CI. shadcn defaults pass; custom CSS must keep
   `data-motion-purpose` attributes.

---

## 2. shadcn-svelte component coverage

The components available at [shadcn-svelte.com/docs/components](https://www.shadcn-svelte.com/docs/components):

```
Accordion, Alert, Alert Dialog, Aspect Ratio, Avatar, Badge, Breadcrumb,
Button, Button Group, Calendar, Card, Carousel, Chart, Checkbox, Collapsible,
Combobox, Command, Context Menu, Data Table, Date Picker, Dialog, Drawer,
Dropdown Menu, Empty, Field, Form, Hover Card, Input, Input OTP, Item, Kbd,
Label, Menubar, Navigation Menu, Pagination, Popover, Progress, Radio Group,
Range Calendar, Resizable, Scroll Area, Select, Separator, Sheet, Sidebar,
Skeleton, Slider, Sonner, Spinner, Switch, Table, Tabs, Textarea, Toggle,
Toggle Group, Tooltip, Typography.
```

### 2.1 Mapping — bespoke `packages/ui` primitives → shadcn-svelte

| Today (`packages/ui/src/lib/components/`)            | Replacement                                                                            |
| ---------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `Button.svelte` (ButtonVariant, ButtonSize)           | `Button` (+ extend `buttonVariants` for our `accent` / `ghost-glow` variants)          |
| `Field.svelte` (text/textarea wrapper)                | `Field` + `Label` + `Input` / `Textarea` + `Form` helpers                              |
| `Chip.svelte`                                         | `Badge` (extend variant set) or `Toggle` for selectable variant                        |
| `Type.svelte` (typography variant)                    | `Typography` (Tailwind utility presets; keep our font-size tokens)                     |
| `Icon.svelte` (wraps lucide)                          | Keep as a thin wrapper around `lucide-svelte` — shadcn uses lucide directly too        |
| `EmptyState.svelte`                                   | `Empty` (new shadcn primitive, perfect 1-1)                                            |
| `StatusBadge.svelte` (status kind)                    | `Badge` with our extended variants + colored dot                                       |
| `Composer.svelte`                                     | Composite using `Textarea` + `Button` + `Tooltip` + our `autoGrow` action              |
| `MessageList.svelte`, `MessageBubble.svelte`          | Custom (no shadcn equivalent). Internals use `Card`, `ScrollArea`, `Avatar`            |
| `ThinkingIndicator.svelte`                            | Use `Spinner` + motion (keep custom)                                                   |
| `ToolCard.svelte`                                     | `Card` + `Badge` + `Collapsible` for expand/collapse                                   |
| `CapabilityCard.svelte`                               | `Card` + `Badge` + `Hover Card`                                                        |
| `PageHeader.svelte`                                   | Composite (`Typography` + `Breadcrumb` + actions slot) — custom on top of shadcn       |
| `DataTable.svelte`                                    | `Data Table` (TanStack Table integration shipped by shadcn)                            |
| `Breadcrumbs.svelte`                                  | `Breadcrumb`                                                                            |
| `AppHeader.svelte`                                    | Composite using `Sidebar.Trigger` + `Breadcrumb` + `DropdownMenu` + `Tooltip`          |
| `Drawer.svelte`                                       | `Drawer` (Vaul-style) — also covers mobile bottom-sheet need                           |
| `Sheet.svelte`                                        | `Sheet`                                                                                 |
| `Sidebar.svelte`, `SidebarSection.svelte`, `SidebarItem.svelte` | `Sidebar.*` (Provider, Root, Header, Content, Group, Menu, MenuItem, MenuButton, …) |
| `AppShell.svelte`                                     | `Sidebar.Provider` + `Sidebar.Inset`                                                    |
| `ThemeProvider.svelte`, `ThemeSwitcher.svelte`        | Keep (still owns our `data-theme` flip + adapter contract). Internals wrap shadcn `DropdownMenu` for the switcher. |
| `ToastHost.svelte` + `toasts` store                   | Replace with `Sonner` — adapt `toasts.push/success/error/warning` to `toast()` API     |
| `PlanBadge.svelte`                                    | `Badge` + variant                                                                       |
| `PlanCard.svelte`                                     | `Card` composition                                                                      |
| `UsageMeter.svelte`                                   | `Progress`                                                                              |
| `QuotaBanner.svelte`                                  | `Alert` (`variant="warning"`) + `Progress`                                              |

### 2.2 Mapping — `features/` and `screens/`

| Today                              | After                                                                              |
| ---------------------------------- | ---------------------------------------------------------------------------------- |
| `AgentChatStream.svelte`            | Internals: `ScrollArea` + new `MessageList` (which uses shadcn `Card`/`Avatar`)    |
| `WorkspaceTree.svelte`              | `Sidebar.MenuSub` + `Collapsible` for the tree, `Context Menu` for actions         |
| `CapabilityBrowser.svelte`          | `Command` (cmd-k palette) + `Tabs` + `Card`                                        |
| `CapabilityRow.svelte`              | `Item` (new shadcn primitive)                                                      |
| `CapabilityPinChip.svelte`          | `Badge` + `Tooltip`                                                                |
| `ContextChip.svelte`                | `Badge`                                                                            |
| `SuggestionChips.svelte`            | `Button Group` (or `Toggle Group`)                                                 |
| `ToolCallCard.svelte`               | `Card` + `Collapsible`                                                             |
| `AttachmentSheet.svelte`            | `Drawer` (mobile) / `Sheet` (desktop) — pick by `is-mobile` hook                   |
| `ProfileSheet.svelte`               | `Sheet` + `Avatar` + `Form`                                                        |
| `DrawerRecentChats.svelte`          | `Drawer` + `Sidebar.MenuSub`                                                       |
| `HostedProjectCard.svelte`          | `Card` + `Badge` + `Tooltip`                                                       |
| `QuotaList.svelte`                  | `Table` (or simple `Item` list) + `Progress`                                       |
| `ShellScreen.svelte`                | Rewritten on `Sidebar.Provider` + `Sidebar.Inset`                                  |
| `ShellPage.svelte`                  | Public API unchanged. Internals composed of new `ShellScreen`                      |
| `ShellLoginScreen.svelte`           | `Card` + `Form` + `Input` + `Button`                                               |
| `screens/ChatScreen.svelte`         | New `MessageList` + `Composer` + `Sidebar.Inset`                                   |
| `screens/CapabilitiesScreen.svelte` | `Command` + `Tabs` + `Data Table`                                                  |
| `screens/CapabilityDetailSheet.svelte` | `Sheet`                                                                          |
| `screens/ArtifactsScreen.svelte`    | `Data Table` + `Tabs`                                                              |
| `features/workspace/ConfirmDialog.svelte` | `Alert Dialog`                                                                |
| `features/workspace/MoveDialog.svelte`    | `Dialog` + `Command` + `Form`                                                 |
| `features/workspace/NewNodeDialog.svelte` | `Dialog` + `Form`                                                              |
| `features/workspace/ShareDialog.svelte`   | `Dialog` + `Form` + `Switch`                                                  |
| `features/billing/InvoiceStatusBadge.svelte` | `Badge` variant                                                            |

### 2.3 Components we do **not** install

`Calendar`, `Range Calendar`, `Date Picker`, `Carousel`, `Chart`, `Input OTP`,
`Menubar`, `Navigation Menu`, `Resizable`, `Pagination` — none used by current
features. Adding later is the standard `pnpm dlx shadcn-svelte add <name>`.

---

## 3. Token bridge

### 3.1 Direction

shadcn-svelte expects CSS custom properties in **OKLCH** named
`--background`, `--foreground`, `--primary`, `--primary-foreground`,
`--secondary`, `--muted`, `--accent`, `--destructive`, `--border`, `--input`,
`--ring`, `--card`, `--popover`, `--sidebar`, `--sidebar-foreground`,
`--sidebar-primary`, `--sidebar-accent`, `--sidebar-border`, `--sidebar-ring`,
plus `--radius`.

[apps/web/src/app.css](../apps/web/src/app.css) already bridges some of these
via `@theme inline { … }`. We extend the same pattern but move it into a
**shared** stylesheet so `apps/browser-shell` also picks it up:

```
packages/ui/src/lib/shadcn-bridge.css      ← new, generated
```

Generated from a new top-level key in `tokens.json`:

```jsonc
{
  "blocks": [ /* existing Foundry blocks */ ],
  "shadcn": {                       // NEW
    "light": {
      "--background": "var(--color-bg)",
      "--foreground": "var(--color-fg)",
      "--card": "var(--color-bg-raised)",
      "--card-foreground": "var(--color-fg)",
      "--primary": "var(--ember)",
      "--primary-foreground": "var(--color-on-accent)",
      "--secondary": "var(--color-bg-raised)",
      "--muted": "var(--color-bg-raised)",
      "--muted-foreground": "var(--color-fg-muted)",
      "--accent": "var(--cyan)",
      "--accent-foreground": "var(--color-fg)",
      "--destructive": "var(--color-danger)",
      "--border": "var(--color-border)",
      "--input": "var(--color-border)",
      "--ring": "var(--ember)",
      "--popover": "var(--color-bg-raised)",
      "--popover-foreground": "var(--color-fg)",
      "--sidebar": "var(--color-bg-raised)",
      "--sidebar-foreground": "var(--color-fg)",
      "--sidebar-primary": "var(--ember)",
      "--sidebar-primary-foreground": "var(--color-on-accent)",
      "--sidebar-accent": "var(--color-bg-hover)",
      "--sidebar-accent-foreground": "var(--color-fg)",
      "--sidebar-border": "var(--color-border)",
      "--sidebar-ring": "var(--ember)",
      "--radius": "var(--radius-md)"
    },
    "forge": { /* dark-mode overrides — usually empty thanks to var() chain */ }
  }
}
```

`scripts/build-tokens.mjs` is extended to emit `shadcn-bridge.css`:

```css
:root,
:root[data-theme="paper"] { /* … shadcn.light values … */ }

:root[data-theme="forge"],
:root.dark              { /* … shadcn.forge overrides … */ }
```

Both `apps/web/src/app.css` and `apps/browser-shell/src/routes/+layout.svelte`
import `@conusai/ui/shadcn-bridge.css` once, alongside `foundry.css`.

### 3.2 Why this direction

- Foundry palette stays the single source of truth — designers edit one JSON.
- shadcn primitives are vendored unmodified; we never re-fork them.
- Theme switch keeps using `data-theme="forge"` — no need to also flip
  shadcn's `.dark` class. (Optionally we set both for parity.)
- Tailwind v4 `@theme inline` continues to project these vars into utility
  classes — `.bg-primary`, `.text-muted-foreground`, `.border-input` etc. all
  work, in both apps.

### 3.3 Tailwind config

`tailwind.config.{ts,js}` is **not** needed in Tailwind v4 — `@theme inline`
in CSS is the new config. `components.json`'s `tailwind.config: ""` field
stays empty.

---

## 4. Where the components live

shadcn-svelte's CLI installs into the path declared in `components.json`. We
move that path **into the shared package** so all apps consume the same files.

```
packages/ui/
├── components.json                         ← new, mirrors apps/web's today
├── src/lib/
│   ├── components/ui/                      ← shadcn-svelte CLI target
│   │   ├── button/
│   │   ├── card/
│   │   ├── sidebar/
│   │   ├── sheet/
│   │   ├── drawer/
│   │   ├── dialog/
│   │   ├── alert-dialog/
│   │   ├── form/
│   │   ├── ... (one folder per installed primitive)
│   ├── components/                         ← bespoke primitives — DELETED after migration
│   ├── features/                           ← rewritten on top of shadcn
│   ├── shadcn-bridge.css                   ← generated
│   └── ...
```

`packages/ui/components.json`:

```json
{
  "$schema": "https://shadcn-svelte.com/schema.json",
  "style": "default",
  "tailwind": { "config": "", "css": "../../apps/web/src/app.css", "baseColor": "zinc" },
  "aliases": {
    "components": "$lib/components",
    "lib": "$lib",
    "utils": "$lib/utils",
    "ui": "$lib/components/ui",
    "hooks": "$lib/hooks"
  }
}
```

> The `tailwind.css` field must point at a stylesheet that imports
> `tailwindcss` — `apps/web/src/app.css` is fine for one-off CLI runs because
> we run the CLI from the workspace root. The actual CSS layer order is owned
> by each app.

`packages/ui/package.json` `exports` adds:

```jsonc
{
  "./components/ui/*": "./src/lib/components/ui/*",
  "./shadcn-bridge.css": "./src/lib/shadcn-bridge.css"
}
```

`apps/web/components.json` is **deleted**. Apps must not re-install components
locally — the workspace CLI script is the only allowed entry point.

```jsonc
// package.json (root) — new script
"ui:add": "pnpm --filter @conusai/ui exec shadcn-svelte add",
"ui:diff": "pnpm --filter @conusai/ui exec shadcn-svelte diff"
```

---

## 5. Tauri / mobile compatibility

shadcn-svelte components are framework-agnostic HTML + Tailwind + Bits UI
(headless primitives). They render in any modern WebView. Specific gotchas:

| Concern                                | Mitigation                                                                                |
| -------------------------------------- | ----------------------------------------------------------------------------------------- |
| WKWebView lacks `:has()` until iOS 15.4 | Min target is iOS 16 — safe.                                                              |
| `oklch()` not supported on Android < 13 WebView | Min Android `minSdkVersion 26`. Detect via `@supports (color: oklch(0 0 0))` and fall back to existing hex tokens — easy because our shadcn vars resolve to Foundry hex/rgba already (we never write raw `oklch()` literals). |
| Bottom-sheet UX                          | shadcn `Drawer` (Vaul) is touch-friendly; preferred over `Sheet` on mobile. `Sheet` for tablet + desktop. Pick via existing `is-mobile.svelte.ts` hook. |
| Sonner positioning on iOS notch         | shadcn `Sonner` honors safe-area-inset via `class` overrides; add `pt-[env(safe-area-inset-top)]` in our `<Toaster>` mount. |
| Keyboard shortcuts                       | shadcn `Sidebar` defaults to `cmd+b` / `ctrl+b`. Change `SIDEBAR_KEYBOARD_SHORTCUT` to match existing `cmd+\` shortcut, or update docs.          |
| Tauri's webview script injection        | The recorder bridge JS in `src-tauri/src/lib.rs` still injects into child tab webviews — unrelated to shadcn.                                  |
| iOS Playwright + the `$state` Map bug   | The fix in `createChatStream.svelte.ts` (toolCardsVersion + toolCardsList) stays. shadcn migration doesn't touch this file.                  |
| Theme propagation to native chrome      | `ThemeProvider`'s `onThemeChange` already emits `theme-change` to Rust. Unchanged.                                                            |

shadcn primitives don't need any platform forks — same source compiles for web,
desktop, iOS, Android.

---

## 6. Phased rollout

Each phase ends with: ✅ green CI (typecheck, lint, vitest, Playwright web,
Playwright iOS, WebDriverIO desktop), ✅ visual regression diff ≤ 0.5 % on
every existing screenshot, ✅ docs updated, ✅ a single PR for review.

### Phase 0 — Foundations (1 sprint)

1. Add `pnpm --filter @conusai/ui exec shadcn-svelte init` and commit the
   resulting `components.json` + Tailwind glue at the package level.
2. Generate `packages/ui/src/lib/shadcn-bridge.css` from `tokens.json` (extend
   `scripts/build-tokens.mjs`). Add it to both apps' CSS entry.
3. Add `Sonner` (`pnpm dlx shadcn-svelte add sonner`) but **don't** wire it up
   yet.
4. Add a runtime feature flag `featureFlags.useShadcn` (default `false`).
5. Add CI: `pnpm --filter @conusai/ui ui:diff` must report zero drift from
   upstream for installed components (prevents accidental forks).
6. Land a parity Storybook-equivalent: extend the `/_/ui` gallery in
   `apps/web/src/routes/_/ui/+page.svelte` with a second registry showing the
   shadcn equivalent next to every Foundry primitive, plus a viewport switcher
   for mobile breakpoints.

**Exit criteria:** Bridge CSS works; both themes still render unchanged;
gallery shows side-by-side parity for every primitive listed in §2.1.

### Phase 1 — Primitive swap (2 sprints)

Order matters: install primitives bottom-up so feature components keep
compiling.

1. `Button`, `Badge`, `Separator`, `Skeleton`, `Tooltip`, `Avatar`, `Label`,
   `Input`, `Textarea`, `Progress` — straightforward swaps. Re-export from
   `packages/ui/src/lib/index.ts` under the same names where possible
   (`export { Button } from "./components/ui/button/index.js"`).
2. Update every consumer of the old `Button`, `Field`, `Chip`, `EmptyState`,
   `StatusBadge`, `Type`, `PlanBadge`, `UsageMeter`, `QuotaBanner` inside
   `packages/ui` to the new exports. Delete the bespoke files.
3. Update `apps/web/src/lib/components/ui/*` — these are the **shadcn
   duplicates** already in the web app. Replace with re-exports from
   `@conusai/ui` and delete the local copies. (Net code reduction.)
4. Run the gallery visual diff. Address any token gap.

**Exit criteria:** zero references to the deleted primitives across the repo.
The cross-app lint blocks re-introducing them. axe-core scores ≥ baseline.

### Phase 2 — Composite chrome (2 sprints)

1. Install `Sheet`, `Drawer`, `Dialog`, `Alert Dialog`, `Dropdown Menu`,
   `Context Menu`, `Popover`, `Tooltip`, `Tabs`, `Collapsible`, `Scroll Area`,
   `Command`, `Hover Card`, `Item`, `Card`, `Alert`, `Form`, `Switch`,
   `Checkbox`, `Radio Group`, `Select`, `Spinner`, `Empty`, `Field`,
   `Typography`, `Breadcrumb`, `Toggle`, `Toggle Group`, `Button Group`,
   `Kbd`, `Table`, `Data Table`.
2. Rewrite the composite chrome — `AppHeader`, `PageHeader`, `Composer`,
   `MessageBubble`, `MessageList`, `ThinkingIndicator`, `ToolCard`,
   `CapabilityCard`, `Breadcrumbs`, `DataTable` — internally using shadcn
   primitives. **Their public prop APIs stay identical** so no caller has to
   change. Add `@deprecated` JSDoc on any prop we plan to drop later.
3. Replace `ToastHost` + `toasts` store with a `Sonner`-backed adapter:
   `toasts.push(t)` → `toast(t.title, { description, action })`. Keep the
   `toasts` named export intact for backward compat.
4. Migrate `features/workspace/*` dialogs to `Dialog` / `Alert Dialog` and
   `Form`.

**Exit criteria:** no internal use of the bespoke composite components remains.
Old composites kept as one-line wrappers around the new ones for one release,
then deleted.

### Phase 3 — Sidebar + ShellScreen (1.5 sprints)

This is the riskiest single change because both apps mount `<ShellPage>` and
deep-link restore + workspace URL sync hang off it.

1. Install `Sidebar` (`pnpm dlx shadcn-svelte add sidebar`).
2. Rewrite `ShellScreen.svelte`:
   - Wrap in `Sidebar.Provider` with `bind:open` reflecting `drawerStore`.
   - Replace `AppShell` + `Sidebar` + `SidebarSection` + `SidebarItem` with
     `Sidebar.Root` + `Sidebar.Content` + `Sidebar.Group` + `Sidebar.Menu` +
     `Sidebar.MenuItem` + `Sidebar.MenuButton`.
   - Use `Sidebar.Inset` for the main pane.
   - Move recents into a `Sidebar.MenuSub` with `Collapsible`.
3. Wire mobile: shadcn `Sidebar` auto-collapses on small viewports via
   `useSidebar().isMobile`. Replace our `is-mobile.svelte.ts` consumers where
   they were used for sidebar state only.
4. Keep `ShellPage`'s public props identical:
   `sdk, chatStream, userName, userPlan, sigil, appTitle, onLogout, onWorkspaceChange, onUnknownRoute`.
5. Update `apps/web/src/routes/+page.svelte` and
   `apps/browser-shell/src/routes/+page.svelte` to flip
   `featureFlags.useShadcn = true` so the new shell renders.

**Exit criteria:** sidebar collapse, mobile drawer, deep-link restore, workspace
URL sync, recents, breadcrumbs all green on web + desktop + iOS + Android E2E.

### Phase 4 — Screens (1 sprint)

1. `screens/ChatScreen.svelte` — new layout using `Sidebar.Inset` +
   `ScrollArea` + new `MessageList` + `Composer`.
2. `screens/CapabilitiesScreen.svelte` — `Command` for search,
   `Tabs` for category, `Data Table` for the list.
3. `screens/CapabilityDetailSheet.svelte` — `Sheet`.
4. `screens/ArtifactsScreen.svelte` — `Data Table` + `Tabs`.
5. `features/ShellLoginScreen.svelte` — `Card` + `Form`.

**Exit criteria:** all screens render through `featureFlags.useShadcn`. Visual
regression and a11y pass.

### Phase 5 — Cleanup (0.5 sprint)

1. Remove `featureFlags.useShadcn` flag and all dual-path code.
2. Delete the legacy bespoke files left in `packages/ui/src/lib/components/`.
3. Delete `apps/web/src/lib/components/ui/` entirely; consumers re-import from
   `@conusai/ui`.
4. Delete `apps/web/components.json`.
5. Drop unused deps: `tailwind-variants` (shadcn uses its own variant helper
   from `bits-ui` and `clsx` / `tailwind-merge`).
6. Update [docs/frontend.md](frontend.md) §4.1 / §6.6 to reflect the new
   layout.
7. Update [docs/ui-design.md](ui-design.md) and
   [docs/ui-inventory.md](ui-inventory.md).
8. Remove the side-by-side shadcn-equivalent panes from `/_/ui` — it is now
   the single source of truth.

**Exit criteria:** repo grep for the old primitive names returns zero hits.
The cross-app lint forbids re-introducing them.

### Phase 6 — Polish (optional, ongoing)

- Install nice-to-haves as needed: `Calendar`, `Date Picker`, `Combobox`,
  `Pagination`, `Carousel`, `Chart`, `Input OTP`.
- Adopt shadcn `Form` everywhere we currently roll our own validation.
- Adopt `Command` palette as a global `cmd-k` action runner.

---

## 7. Concrete CLI commands

Run from the repo root:

```bash
# Phase 0
cd packages/ui
pnpm dlx shadcn-svelte@latest init    # writes components.json
pnpm dlx shadcn-svelte@latest add sonner

# Phase 1
pnpm dlx shadcn-svelte@latest add button badge separator skeleton tooltip avatar label input textarea progress

# Phase 2
pnpm dlx shadcn-svelte@latest add sheet drawer dialog alert-dialog dropdown-menu \
  context-menu popover tabs collapsible scroll-area command hover-card item card \
  alert form switch checkbox radio-group select spinner empty field typography \
  breadcrumb toggle toggle-group button-group kbd table data-table

# Phase 3
pnpm dlx shadcn-svelte@latest add sidebar
```

Every CLI run is followed by:

1. `pnpm --filter @conusai/ui ui:diff` — confirm no manual edits drifted.
2. `pnpm -w build` — confirm both apps still compile.
3. `pnpm -w lint` — Biome + ESLint + the cross-app + design-token guards.
4. `pnpm --filter web test:e2e` and the desktop/iOS/Android suites.

---

## 8. Risks & mitigations

| Risk                                                              | Mitigation                                                                          |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Visual drift in ember/cyan brand color                            | Phase 0 bridge + per-phase Playwright visual diffs across light + dark + iOS + Android. |
| shadcn `Sidebar` keyboard shortcut clash                          | Override `SIDEBAR_KEYBOARD_SHORTCUT` constant; document in `docs/ui-design.md`.     |
| Sonner replaces existing `ToastHost` contract                     | Wrap Sonner behind the existing `toasts.{push,success,error,warning}` API for zero call-site changes. |
| Bits UI `bind:` patterns differ from existing components          | Mostly compatible (Svelte 5 native). Audit `bind:open` / `bind:value` per component during migration; covered by typecheck. |
| Tailwind v4 `@theme inline` conflicts                             | The shadcn bridge runs in the `theme` layer; Foundry tokens stay in the `foundry` layer (lower priority). Cascade order documented in §3. |
| WebKit `oklch()` parity (rare in older OEM Android)               | Tokens resolve to hex; we never emit raw `oklch()` from Foundry. shadcn examples that do are inside vendored components — overridden by the bridge before they ever evaluate. |
| Larger initial JS bundle from `Command` / `Data Table`            | shadcn ships TanStack Table only when `data-table` installed. Code-split per screen via SvelteKit dynamic imports if needed. Add a Lighthouse perf budget to CI. |
| Component fork drift after `shadcn-svelte` updates                | CI `ui:diff` runs weekly and on every PR touching `packages/ui/src/lib/components/ui/`. |
| Asymmetric Svelte 5 + Tauri webview behaviour for `Drawer` (Vaul) | Test on iOS WKWebView and Android WebView in Phase 2 exit gate; fall back to `Sheet` on platforms where touch drag jitters. |
| Inline-svg icon sizing differs between `lucide-svelte` and our wrapper | Keep our `<Icon>` thin wrapper that fixes width/height to `IconSize` token; shadcn primitives accept icons as snippets so this is transparent. |

---

## 9. Lints, CI, and guardrails

New / changed checks:

1. `scripts/check-cross-app-imports.mjs` — extend to also forbid imports of
   shadcn primitives from anywhere other than `@conusai/ui/components/ui/*`.
2. `scripts/check-design-tokens.mjs` — extend to allow shadcn variable names
   (`--background`, `--primary`, …) **only inside**
   `packages/ui/src/lib/components/ui/**` and the generated `shadcn-bridge.css`.
   Everywhere else must use Foundry semantic aliases.
3. New `packages/ui/scripts/check-shadcn-parity.mjs` — runs
   `shadcn-svelte diff` and exits non-zero on local edits inside `components/ui/`.
4. `packages/ui/scripts/check-no-local-components.mjs` — extend whitelist to
   `components/ui/**`.
5. Lighthouse perf budget: web LCP ≤ today's number; bundle delta ≤ +10 % per
   phase.
6. axe-core: zero new violations per phase.

---

## 10. Test strategy

Per phase:

1. **Unit (vitest)** — `pnpm --filter @conusai/ui test`. Re-fixture every
   migrated component (existing `.fixtures.ts` files keep their schema; only
   internals change).
2. **Type (svelte-check)** — `pnpm -w check-types`.
3. **Visual regression (Playwright)** — `apps/web/e2e/visual/*.spec.ts`. Add
   per-phase baselines; reviewer diffs them in PR.
4. **Reduced-motion** — `apps/web/e2e/visual/reduced-motion.spec.ts` already
   exists; extend to cover new components.
5. **Keyboard** — `apps/web/e2e/keyboard.spec.ts` extended for sidebar
   shortcut, command palette, drawer focus trap.
6. **Motion budget** — `motion-budget.spec.ts` + `check-motion-durations.mjs`.
7. **E2E desktop** — `e2e/wdio/` against the macOS Tauri build.
8. **E2E iOS** — Playwright iOS WebKit + `e2e/ios/` Appium specs.
9. **E2E Android** — add a parallel Appium suite under `e2e/android/` if not
   present.
10. **A11y** — `@axe-core/playwright` on every screen.

A merge to `main` requires green on all of the above.

---

## 11. Rollback strategy

Every phase is shipped behind `featureFlags.useShadcn` until Phase 5. To roll
back any phase:

1. Flip the flag default to `false` in `packages/ui/src/lib/stores/featureFlags.svelte.ts`.
2. Redeploy. The old composite paths still exist until Phase 5 deletes them.

After Phase 5 the rollback strategy is `git revert` per-PR.

---

## 12. Estimated effort & sequencing

| Phase            | Effort      | Owner(s)              | Depends on             |
| ---------------- | ----------- | --------------------- | ---------------------- |
| 0 Foundations    | 1 sprint    | platform              | —                      |
| 1 Primitives     | 2 sprints   | platform + design     | 0                      |
| 2 Composites     | 2 sprints   | platform              | 1                      |
| 3 Sidebar+Shell  | 1.5 sprints | platform + mobile     | 2                      |
| 4 Screens        | 1 sprint    | feature owners        | 3                      |
| 5 Cleanup        | 0.5 sprint  | platform              | 4                      |
| 6 Polish         | ongoing     | as needed             | 5                      |

Total: ~8 sprints serialised. Phases 1 & 2 can fan out across multiple
engineers since the primitive swaps are independent.

---

## 13. Definition of done

- [ ] `packages/ui/components.json` installed; bridge CSS generated.
- [ ] Every primitive listed in §2.1 replaced by a shadcn-svelte equivalent
      reachable from `@conusai/ui`.
- [ ] Every feature/screen listed in §2.2 rewritten on shadcn primitives.
- [ ] `apps/web/src/lib/components/ui/` deleted.
- [ ] `apps/web/components.json` deleted.
- [ ] `featureFlags.useShadcn` flag removed.
- [ ] `tailwind-variants` removed from `apps/web/package.json`.
- [ ] CI guards in §9 active.
- [ ] [docs/frontend.md](frontend.md), [docs/ui-design.md](ui-design.md),
      [docs/ui-inventory.md](ui-inventory.md) updated.
- [ ] Playwright web + iOS + Android + WebDriverIO desktop suites green.
- [ ] axe-core score ≥ baseline. Lighthouse perf budget green.
- [ ] No grep hits in repo for the deleted bespoke primitive names.

---

## 14. Appendix — file-by-file checklist (Phase 1 & 2)

Each row maps a deleted file to its replacement; reviewers tick as PRs land.

| Delete (after migration)                                      | Replace with                                                                       | Phase |
| ------------------------------------------------------------- | ---------------------------------------------------------------------------------- | ----- |
| `packages/ui/src/lib/components/Button.svelte`                 | `components/ui/button/`                                                            | 1     |
| `packages/ui/src/lib/components/Field.svelte`                  | `components/ui/field/` + `input` + `textarea`                                      | 1     |
| `packages/ui/src/lib/components/Chip.svelte`                   | `components/ui/badge/` (variant `chip`)                                            | 1     |
| `packages/ui/src/lib/components/Type.svelte`                   | `components/ui/typography/`                                                        | 1     |
| `packages/ui/src/lib/components/EmptyState.svelte`             | `components/ui/empty/`                                                             | 1     |
| `packages/ui/src/lib/components/StatusBadge.svelte`            | `components/ui/badge/`                                                             | 1     |
| `packages/ui/src/lib/components/PlanBadge.svelte`              | `components/ui/badge/`                                                             | 1     |
| `packages/ui/src/lib/components/UsageMeter.svelte`             | `components/ui/progress/`                                                          | 1     |
| `packages/ui/src/lib/components/QuotaBanner.svelte`            | `components/ui/alert/` + `progress/`                                               | 1     |
| `packages/ui/src/lib/components/ToastHost.svelte`              | `components/ui/sonner/` (`Toaster`)                                                | 2     |
| `packages/ui/src/lib/components/Composer.svelte`               | new composite on `textarea` + `button` + `tooltip`                                 | 2     |
| `packages/ui/src/lib/components/MessageBubble.svelte`          | new composite on `card` + `avatar`                                                 | 2     |
| `packages/ui/src/lib/components/MessageList.svelte`            | new composite on `scroll-area`                                                     | 2     |
| `packages/ui/src/lib/components/ThinkingIndicator.svelte`      | `components/ui/spinner/`                                                           | 2     |
| `packages/ui/src/lib/components/ToolCard.svelte`               | `components/ui/card/` + `collapsible/` + `badge/`                                  | 2     |
| `packages/ui/src/lib/components/CapabilityCard.svelte`         | `components/ui/card/` + `hover-card/`                                              | 2     |
| `packages/ui/src/lib/components/PageHeader.svelte`             | new composite on `breadcrumb` + `typography`                                       | 2     |
| `packages/ui/src/lib/components/Breadcrumbs.svelte`            | `components/ui/breadcrumb/`                                                        | 2     |
| `packages/ui/src/lib/components/DataTable.svelte`              | `components/ui/data-table/`                                                        | 2     |
| `packages/ui/src/lib/components/Drawer.svelte`                 | `components/ui/drawer/`                                                            | 2     |
| `packages/ui/src/lib/components/Sheet.svelte`                  | `components/ui/sheet/`                                                             | 2     |
| `packages/ui/src/lib/components/AppHeader.svelte`              | new composite on `sidebar/trigger` + `breadcrumb` + `dropdown-menu`                | 2     |
| `packages/ui/src/lib/components/AppShell.svelte`               | `Sidebar.Provider` + `Sidebar.Inset` (in `ShellScreen`)                            | 3     |
| `packages/ui/src/lib/components/Sidebar.svelte`                | `components/ui/sidebar/`                                                           | 3     |
| `packages/ui/src/lib/components/SidebarSection.svelte`         | `Sidebar.Group` + `Sidebar.GroupLabel`                                             | 3     |
| `packages/ui/src/lib/components/SidebarItem.svelte`            | `Sidebar.MenuItem` + `Sidebar.MenuButton`                                          | 3     |
| `apps/web/src/lib/components/ui/*`                             | re-export from `@conusai/ui`                                                       | 1–5   |

(End of file.)
