# ConusAI — Pixel-Perfect UI Plan

> **Mission:** Take the existing Foundry design system (`packages/ui`) and the two consumers (`apps/web`, `apps/browser-shell`) from "good" to **pixel-perfect, minimal, best-in-class**. No new stack — we double down on the Foundry tokens already shipped (`foundry.css`, `tokens.css`) and ruthlessly enforce them.
>
> **Non‑goals:** Adopting shadcn-svelte / Tailwind v4 (we have a stronger, more opinionated system already). Adding new colors, fonts, or radii. Visual redesigns beyond what's in [`docs/ui-design.md`](ui-design.md) and the screenshots referenced in [`docs/tasks/perfect-ui-task.md`](tasks/perfect-ui-task.md).
>
> **Single source of truth:** `packages/ui` — every screen (web + mobile + desktop Tauri shell) must consume the same primitives. Zero per-app CSS forks.

---

## 0. Operating Principles (read before each phase)

1. **One token, one place.** No hex, no `px` line-heights, no inline shadows outside `tokens.css` / `foundry.css`. **Public canonical tokens use long, descriptive names** — `--color-*` (e.g. `--color-bg`, `--color-fg`, `--color-accent`), `--space-*`, `--radius-*`, `--font-size-*`, `--duration-*`, `--ease-*`. Short forms (`--s-*`, `--r-*`, `--t-*`, `--dur-*`) survive as compatibility aliases (`--s-1: var(--space-1);`) and may be used inside component CSS for brevity, but **net-new tokens land under the long form first**. Brand names (`Forge`, `Paper`, `Ember`) are *theme keys only* — they appear in token definitions (`--color-accent: var(--ember);`) but never in component or prop APIs. See Phase 2.1 for the rename plan.
2. **Mobile-first.** Author at 360 px width; layer up at `≥768`, `≥1024`, `≥1440`. No desktop-only styles without a mobile baseline.
3. **Pixel pass = screenshot diff, tiered by surface.** A single diff threshold across every surface is fantasy — iOS WebView font rendering noise alone exceeds 0.1% routinely. **Every "passes visual diff" reference in this doc resolves to exactly one row below — if a phase quotes a different number, it's a bug, file a fix-the-plan PR.**

   | Surface | `maxDiffPixelRatio` | % | Used by |
   |---|---|---|---|
   | Mechanical / no-UI PRs (renames, exports map, doc) | `0.0001` | 0.01% | Phase 0 audit gate |
   | Primitive in isolation (`/_/ui` gallery shot, 1×/2×/3× DPR) | `0.001` | 0.1% | Phase 2.6, Phase 8 sign-off |
   | Full route screenshot, single platform (web or iOS) | `0.005` | 0.5% | Phase 1.3, Phase 2 token-regen audit, Phase 4, Phase 8 sign-off |
   | Cross-platform perceptual diff (web ↔ iOS ↔ Android, with `[data-platform-chrome]` masked) | `0.02` | 2% | Phase 1.3, Phase 3.6 |

   Below each row, drift is sub-pixel renderer noise; above, it's a real change. Reviewers waiving "just over the threshold" failures is the failure mode that corrupts every gate.
4. **Reduce, don't add.** Every PR must show net-negative or flat LOC in `apps/*/src` once the migration to `packages/ui` is complete.
5. **A11y is a gate.** axe-core 0 violations, WCAG 2.2 AA contrast, full keyboard parity, `prefers-reduced-motion` honored.
6. **One platform-detect path.** `packages/ui/utils/platform.ts` is the only place to branch on iOS/macOS/Android/Windows/web.
7. **No dead code.** Every component shipped is consumed by at least one route in `apps/web` and one screen in `apps/browser-shell`.
8. **Svelte 5 runes only.** All reactivity in `packages/ui` and new code in `apps/*` uses `$state` / `$derived` / `$effect` / `$props` (and `$inspect` in dev). No legacy stores or `let` reactivity in new components. Legacy components migrate on a fixed schedule (see Phase 2.5), not opportunistically — "we'll do it when we touch it" is how you end up with three reactivity styles forever.
9. **Strangler-fig migration.** Never delete a legacy component until its replacement is consumed by **both** `apps/web` **and** `apps/browser-shell` *and* passes its visual + a11y tests. Old + new coexist behind a thin re-export until cutover.
10. **Features vs components, defined.** Three folders, three contracts:
    - **`packages/ui/src/lib/components/`** — **primitives.** Props in, events/callbacks out. No data loading. No workspace/account/capability/artifact domain ownership. Examples: `Button`, `Field`, `Chip`, `Drawer`, `Sheet`, `Icon`, `Type`, `MessageBubble`, `DataTable`, `PageHeader`.
    - **`packages/ui/src/lib/features/`** — **product-specific composed UI.** May know about capabilities, artifacts, accounts, billing, workspace state. Still no direct route ownership. Examples: `CapabilityBrowser`, `ArtifactsScreen`, `QuotaList`, `WorkspaceTree`, `ChatScreen`.
    - **`apps/*/src/routes/`** — **page-level wiring only.** Data loading, routing, auth guards, layout composition. No UI logic worth abstracting; if a route grows a `<style>` block, that's a missing primitive.

    **Classification rule (the test):** does the component's API mention an app-domain noun (workspace, capability, artifact, invoice, plan)? Then `features/`. Otherwise `components/`. A `<Tree items getLabel getIcon getChildren />` belongs in `components/`; the `<WorkspaceTree>` that wraps it and knows what a workspace is belongs in `features/`. New code defaults to `components/` only if it is genuinely data-agnostic.

    **Classification table (canonical decisions for this plan):**
    | Component | Folder | Reason |
    |---|---|---|
    | `Button`, `Field`, `Chip`, `Drawer`, `Sheet`, `Icon`, `Type`, `EmptyState` | `components/` | Generic primitives |
    | `AppShell`, `AppHeader`, `Sidebar`, `SidebarSection`, `SidebarSearch`, `AccountMenuButton` | `components/` | Shell primitives — data passed in |
    | `Composer`, `MessageList`, `MessageBubble`, `ThinkingIndicator`, `ToolCard` | `components/` | Chat primitives — props in, callbacks out |
    | `PageHeader`, `DataTable`, `StatusBadge` | `components/` | Generic page/data primitives. `StatusBadge` takes a generic `status: 'success' \| 'warning' \| 'danger' \| 'neutral'` + `label: string` — knows nothing about invoices. |
    | `InvoiceStatusBadge` (wraps `StatusBadge`) | `features/billing/` | The moment a primitive needs invoice-specific metadata (dates, payment-provider state, retry URL), it stops being generic. Billing-aware wrapper lives here; the wrapped primitive stays generic. |
    | `WorkspaceTree`, `CapabilityBrowser`, `CapabilityRow`, `ArtifactsScreen`, `ArtifactRow`, `QuotaList`, `ProfileSheet`, `WorkspaceCreateMenu`, `AttachmentSheet` | `features/` | Domain language in the name |
    | Stores (`workspace`, `pins`, `theme`, …) | `stores/` (already separate) | Reactive state owners |
11. **Per-file runes opt-in during migration.** Every file being converted gets `<svelte:options runes={true}>` at the top (official Svelte 5 path). Lets the strangler-fig migration proceed one file at a time without flipping the whole compiler mode — mixed legacy/runes trees compile cleanly until the directory-scoped CI gate (Phase 2.5) closes behind us.
12. **Per-phase visual audit on iOS + Web is a merge gate.** No phase exits until it has been visually audited on **both** real consumers: the SvelteKit web build (`apps/web`, Chromium 1280×800 + iPhone 13 viewport) **and** the Tauri iOS shell (`apps/browser-shell` on iOS 18 simulator, iPhone 16 Pro + iPhone SE). Audit = (a) fresh Playwright screenshots for every screen touched in the phase, diffed against the committed baseline **at the tier defined in Principle #3** (full-route shots use the 0.5% row, primitive shots the 0.1% row); (b) axe-core 0 violations on the same routes; (c) a human walk-through against the checklist in [`/.claude/skills/plan-browser-verifier/SKILL.md`](../.claude/skills/plan-browser-verifier/SKILL.md) — hierarchy, contrast, spacing rhythm, type scale, motion, responsive layout, error/empty/loading states, keyboard + focus, safe-area insets. Evidence (screenshots, console log, network log, axe report) attached to the phase PR. **No iOS evidence → no merge**, even if web passes.
   - **Exemption:** PRs that touch *only* this doc, file renames, exports maps, or `package.json` may skip the iOS-sim audit when (a) the web visual diff is `< 0.01%` on every captured route, and (b) the PR carries the `audit-exempt:doc-or-rename` label. The label is the audit trail — no label, no exemption. Used for Phase 0 and any later mechanical-cleanup PR.
13. **Naming, landmarks, and ARIA semantics.** Component names use **common product vocabulary, one name only — no permanent dual-name aliases.** The canonical public set: `AppShell`, `AppHeader`, `Sidebar`, `SidebarSection`, `SidebarSearch`, `AccountMenuButton`, `Drawer`, `Sheet`, `Composer`, `MessageList`, `MessageBubble`, `ThinkingIndicator`, `ToolCard`, `Button`, `Field`, `Chip`, `EmptyState`, `DataTable`, `PageHeader`, `StatusBadge`, `Type`, `Icon`. (Note: only `AppShell` + `AppHeader` carry the `App*` prefix — that prefix is reserved for application-layout infrastructure, not a catch-all when imagination runs out. `Drawer`/`Sheet`/`Composer`/`Sidebar` etc. do not get an `App` prefix.) Brand vocabulary (`Forge`, `Paper`, `Ember`, `Rail`) lives in **tokens and theme files only** — never in component or prop names. (Internal CSS custom properties like `--rail-density` may keep the internal jargon because they describe a layout-density semantic; the *component on disk* is `Sidebar`, and the call site imports `Sidebar`, not `Rail`.) Permanent dual public names are forbidden — they create search friction, docs friction, and import inconsistency.

    **Migration shim:** during the strangler-fig migration, legacy names (`AppTopBar`, `AppDrawer`, `AppBottomSheet`, `AgentChatComposer`, `Rail`, `TopBar`) are re-exported with a JSDoc `@deprecated` tag pointing at the canonical name. The shim is deleted at the migration deadline (end of Phase 4); see anti-pattern entry for the CI gate.

    **Landmarks (WCAG 2.2):** `AppShell` slots carry landmark roles — `<header role="banner">` for the topbar slot, `<nav role="navigation" aria-label="Workspace">` for `Sidebar` when it's primary navigation, `<aside role="complementary">` for any secondary panel, `<main role="main">` for the main slot, `<footer role="contentinfo">` for any footer. **The composer slot is NOT `role="search"`** — a chat composer doesn't behave like search, and labelling it that way mis-leads assistive tech. Use `<form aria-label="Message composer">`. `role="search"` is reserved for actual search/filter inputs (the `<SidebarSearch>` field is a real example).

    **Dialogs:** `Drawer` and `Sheet` are backed by native `<dialog>` plus explicit `aria-modal="true"` and `aria-labelledby` (or `aria-label` if no visible title) — both attributes are TypeScript-required props on the Svelte components; the build fails on omission (see anti-pattern entry).
14. **Motion is communication, not decoration.** Per Apple HIG (motion chapter) and the underlying perception research, every animation must serve exactly one of four purposes — tagged at the call site as `[feedback]` (confirming user action), `[continuity]` (preserving spatial context across state change), `[hierarchy]` (drawing attention to what changed), or `[delight]` (rewarding completion of a multi-step task). An animation that doesn't carry one of these tags doesn't ship. **Per-task animation budget ≤ 3 s** — the sum of all animations on a user-initiated path (e.g. click rail item → screen transitions in → cascade settles) must not exceed three seconds wall-time. Audited by a Playwright assertion that walks the top flows and sums computed `animation-duration` + `transition-duration` on every animated element. `prefers-reduced-motion: reduce` clamps everything to an 80 ms cross-fade and is a hard CI gate, not a soft preference (Principle #5 already states this; #14 says *why*).

15. **Naming conventions, one rule per category.** Boring names scale; clever names are technical debt with a welcome mat.
    - **Svelte components** — `PascalCase.svelte` (`AppShell.svelte`, `MessageBubble.svelte`, `AccountMenuButton.svelte`).
    - **Scripts, docs, generated artifacts** — `kebab-case` (`check-design-tokens.mjs`, `build-tokens.mjs`, `ui-inventory.md`, `ui-landmarks.md`).
    - **Exported code identifiers (functions, classes, types)** — `camelCase` for functions (`detectPlatform`, `tokenSpring`, `createLiveResource`), `PascalCase` for types (`AppShellProps`, `MessageRole`).
    - **CSS custom properties** — long descriptive form, hyphen-separated, semantic-first (`--color-bg`, `--color-accent`, `--space-2`, `--radius-md`, `--duration-fast`). Short aliases (`--s-2`, `--r-md`) are compatibility shims, not the canonical name (per Principle #1).
    - **Component prop variants** — use generic vocabulary: `variant="primary" | "secondary" | "ghost" | "danger"`, `tone="neutral" | "success" | "warning" | "danger"`, `size="sm" | "md" | "lg"`. **Never** `variant="ember"` or `variant="forge"` — brand names belong in tokens, not component APIs.
    - **Event/action props (Svelte 5)** — callback props with `on*` prefix and a `Verb` suffix: `onSelect`, `onDismiss`, `onSubmit`, `onClose`. No legacy `on:event` event dispatch in new components; the `createEventDispatcher` pattern is forbidden after Phase 2.5's runes ratchet closes on `components/`. Bindable props are explicit: `let { open = $bindable(false) }: Props = $props();`.
    - **Platform / capability utilities (per Phase 5.1)** — name by capability, not philosophy: `isTauriRuntime()`, `isIOSWebView()`, `isAndroidWebView()`, `supportsHaptics()`, `supportsSafeAreaEnv()`. Avoid vague identity booleans like `isWeb` — everything is web in a Tauri WebView; the right question is *what can this runtime do*, not *what is it*.

> **Path-shorthand convention used in this doc:** `packages/ui/components/X` is shorthand for the real path `packages/ui/src/lib/components/X` (and same for `features/`, `utils/`, `stores/`, `motion/`). Don't change the on-disk layout to match the shorthand — the SvelteKit `src/lib` boundary is load-bearing for the package's exports.

---

## Release packaging — three mergeable milestones, not nine abstract phases

Phase numbering is reading order (and matches existing PR titles / branch names). For **planning and shipping**, the phases roll up into three releases, each with a single clear exit criterion. Don't ship Phase 4 before Release A is closed — primitives without locked tokens become a refactor target by the time Release C lands.

| Release | Bundles | Exit criterion |
|---|---|---|
| **Release A — Foundation lock** | Phase 0, Phase 1, Phase 2.1 (tokens), Phase 2.5 (runes for primitives), Phase 2.6 (primitive gallery), Phase 2.7 (cross-cutting primitives) | Token audit passes. Primitive gallery exists at `/_/ui`. Component naming is frozen per Principles #13 + #15. Visual + a11y baselines committed. Zero new app-local CSS. |
| **Release B — Shell unification** | Phase 2.2 (typography), 2.3 (motion), 2.4 (icons), Phase 3 (entire — `AppShell`, `Drawer`/`Sheet`, `AppHeader`, `Sidebar`, `Composer`), Phase 3.6 (Android smoke) | `apps/web` and `apps/browser-shell` consume the same `AppShell`. `MobileShell.svelte` deleted. The 7 `apps/browser-shell/src/lib/mobile/parts/*` files resolved (moved, folded, or deleted per §0.1 disposition table). Keyboard / safe-area / drawer behavior tested on web + iOS sim + Android emulator. |
| **Release C — Screen pixel pass + ship gate** | Phase 4 (screens 4.1 → 4.10), Phase 5 (native polish), Phase 6 (motion), Phase 7 (a11y/i18n), Phase 8 (sign-off) | Zero local CSS in `apps/*`. Every screen passes the Principle #3 thresholds on web + iOS + Android. All CI gates from §8.1 (lint, test, visual, a11y, exports, no-local-components, **ui:contracts**) enforce the rules so humans don't have to be the linter. |

Within each release, sub-phases ship as their own PRs (per Phase 4's PR cadence rule) — the release boundary is just the merge-train milestone where the next release can start.

---

## Phase 0 — Reconcile (one-day PR, no functional change)

**Goal:** Get the on-disk surface and this plan into agreement *before* any audit script or visual baseline runs against them. Everything that follows assumes a clean foundation.

**Why this exists:** A prior audit (2026-05-23) found this plan referenced paths and assets that don't match reality — e.g. `AppDrawer` was claimed under `apps/browser-shell/src/lib/mobile/` but actually lives in `packages/ui/src/lib/features/chrome/`; typography was specified as Fraunces/Switzer/JetBrains Mono but `foundry.css` self-hosts Geist/Geist Mono; the `features/` vs `components/` boundary was unstated; no `package.json` exports map governs the primitives the later phases assume. Fixing these in flight bloats every subsequent PR. Fix once, here.

**Execution order inside Phase 0** (sub-section numbering is reading order, not exec order — same convention as the overall §"Execution order" at the end of this doc):

1. **§0.4 — Typography lock first.** The font choice is the most visible identity decision; nothing else in Phase 0 references its outcome but every later phase does, so closing it before touching the exports map prevents a re-edit.
2. **§0.3 — Token dedup.** Remove the duplicate `--ink-*` definitions from `foundry.css` so the Phase 1.2 token-audit script has a clean target.
3. **§0.1 — Move chrome primitives.** `git mv` the four files into `components/`. Pure renames, no behavior change.
4. **§0.2 — Extend exports map.** Adds the per-file `./components/*` shape on top of the existing `./capabilities` / `./live` / `./stores` / `./utils` / `./features` / `./motion` entries (which §0.2 must **preserve, not replace**). Runs after §0.1 because the map describes the file locations §0.1 produces.
5. **§0.5 — Fix factual errors in this doc.** Sweep-up edits that depend on the moves above.
6. **§0.6 — Reconcile [`docs/tasks/perfect-ui-task.md`](tasks/perfect-ui-task.md).** Last because it's a sibling doc update, not load-bearing on code.

### 0.1 Move chrome primitives into `components/`
- [ ] `git mv packages/ui/src/lib/features/chrome/AppTopBar.svelte packages/ui/src/lib/components/AppTopBar.svelte`
- [ ] `git mv packages/ui/src/lib/features/chrome/AppDrawer.svelte packages/ui/src/lib/components/AppDrawer.svelte`
- [ ] `git mv packages/ui/src/lib/features/chrome/AppBottomSheet.svelte packages/ui/src/lib/components/AppBottomSheet.svelte`
- [ ] `git mv packages/ui/src/lib/features/AgentChatComposer.svelte packages/ui/src/lib/components/AgentChatComposer.svelte` *(generic-enough; will be renamed to `Composer` in Phase 3.5)*
- [ ] Keep the `App*` names in this PR — renames to the canonical `AppHeader` / `Drawer` / `Sheet` / `Composer` (per Principle #13) happen in Phase 3 as the strangler-fig API change, not a mechanical move.
- [ ] **`@deprecated` JSDoc requirement (Principle #13 migration shim).** The instant a canonical replacement lands in a later phase, the moved `App*` file gains a JSDoc header pointing at the new name, e.g.:
  ```svelte
  <!--
   @deprecated Use Composer.svelte instead.
   Kept only as a migration shim — removed at the Phase 4 close gate.
  -->
  ```
  Plus a runtime dev-mode warning at first mount: `if (import.meta.env.DEV) console.warn('[deprecated] AgentChatComposer → import Composer from @conusai/ui/components/Composer.svelte');`. Without this, the legacy names will outlive the migration because someone will say "it works, why touch it?" Make leaving them in *louder* than removing them.
- [ ] **Sunset deadline:** all `@deprecated` shims are deleted at the **end of Phase 4** (Release C interior boundary). The deletion is enforced by the `pnpm ui:contracts` gate in §8.1.

**Inventory: app-local components that violate Principle #7 (no app-local UI in `apps/*`).** Audit (2026-05-23) found 7 helper files in `apps/browser-shell/src/lib/mobile/parts/` plus the 621-line `MobileShell.svelte` (see §0.5 and §3.1) that the original plan did not list. **Phase 0 does not move them** — they have app-specific shape (drawer composition, profile-sheet flow) that Phase 3 needs to redesign. The work here is to **enumerate them so they can't be forgotten** when Phase 3.1 (`AppShell`), 3.2 (`Drawer`/`Sheet`), 3.4 (`Rail` / `WorkspaceTree` dedup) land. Disposition decided in those phases:

  | File | Phase that absorbs it | Likely fate |
  |---|---|---|
  | `mobile/MobileShell.svelte` | 3.1 | Deleted — replaced by `AppShell` consumption in `+layout.svelte` |
  | `mobile/parts/ProfileSheet.svelte` | 3.2 | Move to `packages/ui/src/lib/features/` as `ProfileSheet`; consume new `Sheet` primitive |
  | `mobile/parts/AttachmentSheet.svelte` | 3.5 (composer attachments) | Move to `features/` as `AttachmentSheet`; consume `Sheet` + `Chip` primitives |
  | `mobile/parts/WorkspaceCreateMenu.svelte` | 3.2 | Move to `features/` as `WorkspaceCreateMenu`; consume `Sheet` |
  | `mobile/parts/DrawerWorkspaceTree.svelte` | 3.4 | Deleted — folded into the unified `WorkspaceTree` (driven by `--rail-density`) |
  | `mobile/parts/DrawerProfileHeader.svelte` | 3.4 | Move to `components/` as part of the `Sidebar` family (`AccountMenuButton` consumes it) |
  | `mobile/parts/Breadcrumbs.svelte` | 3.3 | Move to `components/` as `Breadcrumbs` primitive; consumed by `AppHeader` slot |
  | `mobile/parts/WorkspaceTreeRow.svelte` | 3.4 | Folded into the unified `WorkspaceTree`'s row implementation |

- [ ] Land this table in the Phase 0 PR (no moves yet) so reviewers and later-phase PRs reference one canonical disposition list, not an evolving one.

### 0.2 Extend package exports map
- [ ] **Extend, do not replace.** The current `packages/ui/package.json` already exports `.` / `./assets/*` / `./tokens.css` / `./foundry.css` / `./capabilities` / `./stores` / `./utils` / `./features` / `./motion` / `./live` — barrel-only shapes that consumers depend on today. Adding per-file `./components/*` lets new code import `import AppShell from '@conusai/ui/components/AppShell.svelte'` without breaking the existing barrels. Final shape:
  ```json
  "exports": {
    ".":               { "types": "./src/lib/index.ts", "svelte": "./src/lib/index.ts" },
    "./assets/*":      "./src/lib/assets/*",
    "./tokens.css":    "./src/lib/tokens.css",
    "./foundry.css":   "./src/lib/foundry.css",
    "./components/*":  "./src/lib/components/*",
    "./features/*":    "./src/lib/features/*",
    "./utils/*":       "./src/lib/utils/*",
    "./stores/*":      "./src/lib/stores/*",
    "./capabilities":  { "types": "./src/lib/capabilities/index.ts" },
    "./motion":        { "types": "./src/lib/motion/index.ts" },
    "./live":          { "types": "./src/lib/live/createLiveResource.svelte.ts" }
  }
  ```
  Barrel exports (`./capabilities`, `./motion`, `./live`) stay as objects with `types` keys because their consumers import named symbols, not files. The per-directory wildcards (`./components/*`, `./features/*`, `./utils/*`, `./stores/*`) are net-new and unlock the deep-file imports later phases assume.
- [ ] Update import statements in `apps/web` and `apps/browser-shell` to the new paths (find/replace; CI will catch regressions). **Svelte component imports use explicit `.svelte` extensions** (`from '@conusai/ui/components/Button.svelte'`) unless an index barrel re-exports them as a named symbol — extensionless `.svelte` imports work in some bundler configs and break in others; explicit is the only portable shape and matches what `test:exports` verifies.
- [ ] **Verify the exports map resolves for TypeScript consumers** before merging:
  ```bash
  pnpm --filter @conusai/ui build && pnpm -w exec svelte-check
  ```
  Fails fast if any deep import path is missing from the map or any `./components/*` entry has no matching file.

### 0.3 Deduplicate tokens
- [ ] **Audit (2026-05-23) correction:** `--ink-2` / `--ink-3` are defined in **both** `foundry.css` (light theme lines 27–28, dark theme lines 38–39) and `tokens.css`. The work is **delete the foundry.css copies**, leaving `tokens.css` as the single definer. `foundry.css` should only consume tokens, never define them. Pre-empts the Phase 1.2 token-audit script catching the duplication.
- [ ] After deletion, grep the repo for any other `--ink-*` definitions outside `tokens.css` — there should be zero. Same check for any token with a `tokens.css` definition.

### 0.4 Reconcile typography choice  *(executes FIRST inside Phase 0 — see top-of-phase exec order)*
- [ ] **Audit (2026-05-23) correction:** the decision is **already locked**. [`packages/ui/src/lib/foundry.css`](../packages/ui/src/lib/foundry.css) self-hosts Geist Variable + Geist Mono Variable (via `assets/fonts/`); [`docs/ui-design.md`](ui-design.md) lists Geist as final and marks Fraunces / Switzer / JetBrains Mono as "retired — do not reintroduce." No content reconciliation needed; the remaining work is the sentinel below.
- [ ] Drop a sentinel comment at the top of `foundry.css` so the choice can't drift silently: `/* Typography locked to Geist + Geist Mono per docs/ui-plan.md Phase 0.4 */`. This is the **single deliverable of §0.4**, and it has to land before §0.2 because the exports map's `./foundry.css` entry is the public surface for consumers reading that sentinel.

### 0.5 Fix this doc's factual errors
- [ ] **AppShell.svelte characterization.** Audit (2026-05-23): the file is 930 bytes / 51 lines, **already on Svelte 5 runes** (`$props()`), and renders a 2-column flex layout with `sidebar` + `children` snippets backed by `--paper` / `--ink` / `--rule` tokens. It is a working v0, **not a stub**. The Phase 3.1 work is still a **rewrite** (it has no breakpoints, no container queries, no `topbar`/`composer`/`overlay` slots, no `Drawer` integration, no landmark roles) — but the wording in §3.1 must say "discardable v0, rewriting against the slot contract" rather than "build from scratch." Treat the current contents as informative for token usage (the new shell can mirror its color binding), nothing else.
- [ ] **MobileShell path + size correction.** Plan §3.1 originally cited `apps/browser-shell/src/lib/MobileShell.svelte` at 235 lines. Actual file is at [`apps/browser-shell/src/lib/mobile/MobileShell.svelte`](../apps/browser-shell/src/lib/mobile/MobileShell.svelte) and is **621 lines** — almost 3× larger. The §3.1 path and line count below are now corrected; this checkbox is the audit trail.
- [ ] Plan §3.2 referenced `apps/browser-shell/src/lib/mobile/AppDrawer.svelte` — corrected to `packages/ui/src/lib/components/AppDrawer.svelte` (after the 0.1 move).
- [ ] **Phase 4.8 — ArtifactsScreen / ArtifactRow status (audit re-check 2026-05-23).** Original plan said "these already exist." First audit pass said MISSING. **Re-check found both files at [`packages/ui/src/lib/features/screens/ArtifactsScreen.svelte`](../packages/ui/src/lib/features/screens/ArtifactsScreen.svelte) and [`features/screens/ArtifactRow.svelte`](../packages/ui/src/lib/features/screens/ArtifactRow.svelte)** — the first audit missed the `features/screens/` subdirectory. §4.8 is corrected back to "files exist, swap their sheet wrapper for the canonical `<Sheet>`"; this checkbox is the audit trail. **Lesson:** future `dump-ui-inventory.mjs` script (§1.1) must recurse into all sub-directories.
- [ ] **Phase 2.5 — runes adoption baseline.** Plan originally claimed "0% adoption in components." Audit found `AppShell.svelte` is the lone exception — already on `$props()`. Baseline is **10% (1 of 10) in components; 0% in features; 100% in stores**. §2.5 is corrected below.

### 0.6 Reconcile [`docs/tasks/perfect-ui-task.md`](tasks/perfect-ui-task.md) with this plan
- [ ] **Conflict:** the task brief recommends **Tailwind v4 + shadcn-svelte + Tauri v2 + SvelteKit + Svelte 5** as the stack and references shadcn primitives by name (sidebar, sheet, drawer, button, card, input, avatar, badge, scroll-area). This plan's **Non-goals** explicitly reject Tailwind v4 and shadcn-svelte in favor of the in-repo Foundry system (`tokens.css` / `foundry.css` / `packages/ui`). Two docs disagreeing about the foundational stack will cause every later phase to absorb the wrong primitives.
- [ ] **Resolution:**
  1. Prepend a banner to [`docs/tasks/perfect-ui-task.md`](tasks/perfect-ui-task.md):
     ```markdown
     > **Status:** Stack recommendations superseded by [`docs/ui-plan.md`](../ui-plan.md) (2026-05-23).
     > The reference screenshots and screen-by-screen UX critique below remain authoritative — the **stack** (Tailwind v4 / shadcn-svelte) does not. Pixel-pass against the screenshots using the in-repo Foundry system (`packages/ui` + `tokens.css` / `foundry.css`), not shadcn primitives.
     ```
  2. Strike or comment out any prescriptive "install shadcn `sidebar`" / "use Tailwind v4 `@apply`" instructions in that file; leave the visual review and screenshot references intact.
  3. **Do not delete the file** — it carries the screenshots and the per-screen critique that Phase 4 consumes. The banner is the contract; the rest is reference.

**Exit criteria:** Both apps build and pass current tests with the new import paths. No visual changes. PR diff is almost entirely renames + `package.json` + this doc + the [`docs/tasks/perfect-ui-task.md`](tasks/perfect-ui-task.md) banner.

**Visual audit gate (Principle #12, with exemption):** Smoke-render `/` and `/login` on web only (Chromium 1280×800 + iPhone 13). Web diff against `main`'s baseline must be `< 0.01%` — anything larger means a rename or exports-map change silently altered output; block and fix. Apply the `audit-exempt:doc-or-rename` label per Principle #12 to skip the iOS-sim run; this PR is mechanical and iOS-renderer noise would dominate any real signal.

---

## Phase 1 — Audit & Baseline (foundation)

**Goal:** Know exactly where we stand before changing pixels.

### 1.1 Inventory
- [ ] Enumerate every component in `packages/ui/src/lib/{components,features,capabilities}` → write to `docs/ui-inventory.md` (auto-gen script in `scripts/dump-ui-inventory.mjs`).
- [ ] Enumerate every route in `apps/web/src/routes/**` and `apps/browser-shell/src/routes/**` with: screen name, primary container, which shared component it should render.
- [ ] List every `style` / `<style>` block in `apps/*` — these are violations of the shared‑UI rule. Track count as a regression metric.

### 1.2 Token audit (automated)
- [ ] Add `scripts/check-design-tokens.mjs`:
  - Fails CI on raw hex outside `packages/ui/src/lib/{tokens,foundry}.css`.
  - Fails CI on `px` values in `padding|margin|gap|font-size|line-height|border-radius` outside the token files.
  - Fails CI on `cubic-bezier(` / `transition: .*ms` outside motion tokens.
  - Fails CI on any `style:` Svelte directive **or** `<style>` block in `apps/*` containing color/radius/size literals (catches inline drift the other rules miss).
- [ ] Wire into `pnpm lint` and `turbo` pipeline.

### 1.3 Visual regression baseline
- [ ] Stand up Playwright visual tests in `apps/web/e2e/visual/`:
  - Routes: `/login`, `/`, `/account`, `/account/billing`, `/account/usage`, error state.
  - Viewports: `360×780` (iPhone SE), `390×844` (iPhone 16), `768×1024` (iPad), `1280×800` (laptop), `1680×1050` (desktop).
  - Both `paper` and `forge` themes.
  - **Run inside the official `mcr.microsoft.com/playwright` Docker image** in CI and locally (via `just visual`) — guarantees byte-identical font rendering across macOS/Linux/Windows contributors.
  - **Audit (2026-05-23):** the root `justfile` has no `visual` recipe today. Add one as part of this sub-phase:
    ```make
    # justfile — Phase 1.3
    visual:
        docker run --rm --network host -v $PWD:/work -w /work \
          mcr.microsoft.com/playwright:v1.49.0-jammy \
          pnpm --filter web exec playwright test --grep visual
    ```
    Without the recipe the locally-vs-CI font-rendering guarantee in this bullet is aspirational.
  - **Mask dynamic regions** (`mask: [page.locator('[data-volatile]')]`) on timestamps, "just now" labels, user names, streaming cursors, generated IDs.
  - Snapshot threshold per Principle #3 tiering: `maxDiffPixelRatio: 0.001` for primitive gallery shots, `0.005` for full-page route screenshots (the route-level set lives here in 1.3). Cross-platform diff (next sub-bullet) keeps its own 2% threshold. Store under `e2e/__screenshots__/{project}/{theme}/{viewport}/`.
- [ ] Mirror for `apps/browser-shell` via WDIO/Tauri driver in `e2e/wdio/` and `e2e/ios/`.
- [ ] Commit current screenshots as the "before" baseline under `test-results/visual-baseline-2026-05/`.
- [ ] Add a **`/approve-snapshots` PR command** (GitHub Actions workflow): when a maintainer comments `/approve-snapshots`, CI re-runs Playwright with `--update-snapshots`, commits the diff back to the PR branch. Eliminates "works on my machine" snapshot churn.
- [ ] **Cross-platform perceptual diff:** add `scripts/cross-platform-diff.mjs` that, for each route in the audit matrix, loads the corresponding web screenshot (Chromium iPhone 13 viewport) and iOS-sim screenshot (iPhone 16 Pro), masks the documented platform-chrome regions (`[data-platform-chrome]` selector — status bar, home indicator, browser address bar, traffic-lights), and runs a perceptual diff (`pixelmatch` or `odiff`). Fails CI if the unmasked-region diff exceeds 2% — that's the threshold above which the two surfaces have meaningfully diverged. Catches "looks fine on web, broken on iOS" before manual audit.

### 1.4 Lighthouse + axe
- [ ] **Audit (2026-05-23):** `@axe-core/playwright` is **not yet installed** in `apps/web/package.json`. First step of this sub-phase: `pnpm --filter web add -D @axe-core/playwright lighthouse`. The `pnpm ui:audit` script below assumes both are present.
- [ ] Add `pnpm ui:audit` running Lighthouse (mobile preset) and `@axe-core/playwright` against the web app. Snapshot scores; gate further phases on no regression.
- [ ] **Audit consolidator:** add `scripts/audit-phase.mjs <phase>` that runs every gate applicable to the given phase (visual, axe, cross-platform diff, motion budget once Phase 6 lands, size budget) and emits a single PR-ready artifact bundle (`test-results/audit-phase-N/`) containing screenshots, reports, and a one-page Markdown summary. Drops the contributor cost of the per-phase audit gate (Principle #12) from "assemble 6 artifacts by hand" to `pnpm audit:phase 3`. Without this, the audit gate gets skipped because it's tedious — same failure mode every comprehensive QA process eventually hits.

### 1.5 Shared definitions referenced by later phases
- [ ] **Top-5 task paths** — define once here so Phases 6 and 7 reference one canonical list, not two drifting ones. These are the user-initiated paths that motion-budget audits, VoiceOver walks, and reduced-motion tests all walk:
  1. `login` → `/` (cold start to greeting)
  2. `send message` (composer focus → submit → first token rendered)
  3. `open capability detail` (rail item → capability browser → detail pane/sheet)
  4. `open artifact preview` (artifacts grid → row click → preview sheet/panel)
  5. `change theme` (rail user chip → theme switcher → repaint settled)

  Export as `e2e/fixtures/task-paths.ts` so Playwright specs import the canonical definitions instead of redefining selectors.

**Exit criteria:** CI green, baseline screenshots committed, inventory + violation count published, task-paths fixture in place.

**Visual audit gate (Principle #12):** This phase *establishes* the baseline — every route in the audit matrix must have a committed screenshot on web (Chromium desktop + iPhone 13) **and** Tauri iOS simulator (iPhone 16 Pro + iPhone SE). Phase exits only when the baseline set is reviewable as a PR artifact (gallery comment with all images) and the iOS set visually matches the web set modulo expected platform chrome (status bar, safe-area, keyboard).

---

## Phase 2 — Token & Foundation Hardening

**Goal:** Lock the design primitives so every later step is a pure consumer.

### 2.1 Token completeness
- [ ] **Public canonical names are long-form** (per Principle #1 + #15). Define the canonical set in `tokens.css` first; short aliases (`--s-*`, `--r-*`, `--t-*`, `--dur-*`) become *one-liners that reference the canonical* — not the other way around. Example shape:
  ```css
  :root {
    /* canonical */
    --space-1: 4px;  --space-2: 8px;  --space-3: 12px;  --space-4: 16px;  /* … */
    --radius-sm: 6px;  --radius-md: 12px;  --radius-lg: 18px;
    --font-size-body: 15px;  --font-size-label: 13px;  --font-size-display: 32px;
    --duration-fast: 120ms;  --duration-normal: 200ms;  --duration-slow: 320ms;

    /* compatibility aliases — survive only because existing CSS uses them */
    --s-1: var(--space-1);  --s-2: var(--space-2);  /* … */
    --r-sm: var(--radius-sm);  --r-md: var(--radius-md);  --r-lg: var(--radius-lg);
    --t-body: var(--font-size-body);  --t-label: var(--font-size-label);  --t-display: var(--font-size-display);
    --dur-1: var(--duration-fast);  --dur-2: var(--duration-normal);  --dur-3: var(--duration-slow);
  }
  ```
- [ ] Review `packages/ui/src/lib/tokens.css` against [`docs/ui-design.md`](ui-design.md) §2–§6. Add any missing semantic tokens (hit-target min size `--hit: 44px`, safe-area `--safe-*`, sidebar widths `--sidebar` / `--sidebar-collapsed: 64px` *(per Principle #13 — `Sidebar` not `Rail` in the public token name)*, composer max `--composer-w`, container-query names `--container-app-shell` / `--container-sidebar`, focus ring `--focus-ring`).
- [ ] Add **semantic color aliases** under the canonical naming: `--color-bg`, `--color-bg-raised`, `--color-bg-hover`, `--color-fg`, `--color-fg-muted`, `--color-border`, `--color-border-strong`, `--color-accent`, `--color-accent-hover`, `--color-danger`, `--color-success`. Components reference **only** semantic aliases (`background: var(--color-bg-raised)`); theme files (Forge / Paper) remap them to brand tokens (`--color-accent: var(--ember)`). Brand names (`--ember`, `--paper`, `--forge`) remain as theme-internal scalars — never used directly in component CSS.
- [ ] **Existing short-token consumers stay valid** — the aliases above are real `var(...)` indirections, not removals. Phase 2.1 does not rewrite every `var(--s-2)` in the codebase; that's the codemod in the next bullet, scheduled at Phase 2.1 close once the alias layer ships.
- [ ] **Token rename ships as a sequence of small PRs, NOT one mega-PR** — combining schema change + repo-wide rewrite in one diff turns "simple token cleanup" into a 900-file unreviewable monster. Sub-phase execution:
  - **§2.1a** — *Add long-form tokens + aliases.* New canonical `--color-*` / `--space-*` / `--radius-*` / `--font-size-*` / `--duration-*` land in `tokens.css`; short forms are rewritten as `var(--...)` aliases. **No consumer code changes.** Diff is purely `tokens.css`. Visual diff must be exactly `< 0.01%` (mechanical PR tier) — any larger means the `var(...)` indirection changed a computed value.
  - **§2.1b** — *Regenerate token outputs + parity test.* If the tokens-as-code generator from §2.1 is wired, run it; commit any regenerated `tokens.css` / `tokens.d.ts`. Token-parity unit test (theme keys match across Paper/Forge) ships here.
  - **§2.1c** — *Run codemod.* `scripts/rename-token.mjs --short-to-long` rewrites every `var(--s-2)` → `var(--space-2)` etc. across `packages/ui/**` and `apps/**`. Aliases stay in `tokens.css` as safety net. Diff is pure mechanical rewrite — large file count, zero logic changes.
  - **§2.1d** — *Visual diff sweep.* Run full Playwright visual baseline against the codemod commit. Must pass at the §1.3 "full route" tier (0.5%). Any drift → bisect to the offending var.
  - **§2.1e** — *Turn short-form usage to warning in `scripts/check-design-tokens.mjs`.* From this point, new code can't use `--s-*` / `--r-*` / `--t-*` / `--dur-*`. Existing usage was rewritten in §2.1c so the warning count is zero on `main`; any new occurrence trips CI.
  - **Phase 4 close** — *Warning → error.* Aliases stay in `tokens.css` indefinitely as a safety net for any third-party CSS that snuck in, but the linter treats them as forbidden.
- [ ] **Tokens-as-code:** introduce `packages/ui/tokens/tokens.json` as the single source of truth + a generator that emits `tokens.css`, `tokens.d.ts`, and a JSON snapshot for design tooling. **Default to a bespoke `scripts/build-tokens.mjs`** (~30 lines, zero deps) — our current token count is ~80, no Figma-sync need exists, no multi-platform output (iOS Swift / Android XML) is on the roadmap. Document the JSON schema as the contract. Escalate to **Style Dictionary** (+ `@tokens-studio/sd-transforms`) only when one of those three drivers actually shows up — at which point the JSON schema is the migration boundary, not a rewrite. Hand-editing `tokens.css` becomes forbidden after this lands. The generator file starts with a sentinel header comment so the escalation criteria can't drift:
  ```js
  /*
   * tokens.json → tokens.css/.d.ts generator.
   * Bespoke (zero-dep) by design — our token count is ~80, web-only, no Figma sync.
   * ESCALATE to Style Dictionary + @tokens-studio/sd-transforms ONLY when one of:
   *   (1) Figma → tokens sync is on the roadmap
   *   (2) Native iOS Swift / Android XML output is needed
   *   (3) Token count exceeds ~200
   * The tokens.json schema is the migration boundary; nothing else changes.
   */
  ```
- [ ] **Token-rename codemod:** add `scripts/rename-token.mjs <old> <new>` that updates `tokens.json`, regenerates outputs, runs a repo-wide ripgrep replace across `.css` / `.svelte` / `.ts` / `.tsx`, and writes a one-line entry to `docs/ui-tokens-changelog.md` (date, old, new, PR). Without this, semantic-alias migration in 2.1 becomes a 200-file manual find/replace.
- [ ] Verify Forge (dark) and Paper (light) coverage is 1:1 — write a unit test that diffs the token keys.

### 2.2 Typography pipeline
- [ ] Confirm Geist + Geist Mono (already self-hosted via `foundry.css`) are preloaded in `app.html` of each app with `<link rel="preload" as="font" crossorigin>` for the two most-used weights. (Geist replaces the earlier Fraunces/Switzer/JetBrains Mono direction — see Phase 0.4.)
- [ ] Add a `<Type variant="display|h1|h2|label|meta|mono">` primitive in `packages/ui/components/typography/` for **headings, display, labels, meta, and code** only. Body copy uses semantic elements (`<p>`, `<li>`, etc.) with `class="t-body"` / `t-body-strong` token classes — wrapping every `<p>` in `<Type variant="body">` creates component soup with no benefit. The discipline is "no inline `font-*` declarations and no untokenized `font-variation-settings`," not "everything must be a component."
- [ ] `<Type>` is the **only** place `font-variation-settings` lives (Geist is a variable font — the display/h1/h2 variants set `wght` + `opsz` axes; body token classes get a flat weight). Zero inline copies of `font-variation-settings` anywhere else in the repo.

### 2.3 Motion primitives
- [ ] Consolidate animations from `packages/ui/src/lib/motion/` into named keyframes: `msg-in`, `msg-in-user`, `dot-wave`, `cursor-pulse`, `card-flash-success`, `card-flash-error`, `view-fade-in`, `toast-in`, `cascade-in`.
- [ ] Export Svelte `transition:` helpers (`fadeRise`, `slideFromRight`, `cascade(delayMs)`) so app code never writes `transition` strings.
- [ ] Wrap every animation with `@media (prefers-reduced-motion: reduce) { animation-duration: var(--dur-1) !important; }`.
- [ ] **Named easing curves** in `tokens.css` — match the Material 3 / iOS HIG vocabulary so component authors aren't inventing curves. Note: M3's "emphasized" is the same `cubic-bezier(0.2, 0, 0, 1)` as "standard" in CSS — the real differentiation comes from springs (next bullet) or the directional decelerate/accelerate variants. So we expose **only the variants that have distinct CSS values**, plus springs for everything M3 would model as emphasized:
  ```css
  --ease-standard:                cubic-bezier(0.2, 0, 0, 1);       /* default — all state changes that don't enter/leave the viewport */
  --ease-emphasized-decelerate:   cubic-bezier(0.05, 0.7, 0.1, 1);  /* element entering screen / appearing */
  --ease-emphasized-accelerate:   cubic-bezier(0.3, 0, 0.8, 0.15);  /* element leaving screen / disappearing */
  --ease-linear:                  linear;                            /* opacity / colour fade only — never positional */
  ```
  Each curve has a documented use case as a comment in `tokens.css` — pick by purpose, not by feel. For anything M3 would tag "emphasized" (hero sheet open, screen morph) use `--spring-snappy` or `--spring-gentle` instead — springs give that quality the cubic-bezier can't.
- [ ] **Spring tokens** for native-feeling rail collapse, sheet drag, hover lift — `cubic-bezier` can't model physics. Triplet `[stiffness, damping, mass]` consumed by the existing `springAnimate` helper:
  ```css
  --spring-snappy: 380 30 1;   /* default for primary controls (composer send rebound) */
  --spring-gentle: 170 26 1;   /* secondary controls (rail expand, sheet snap) */
  --spring-bouncy: 280 14 1;   /* delight-tagged only (success confetti, badge unlock) */
  ```
- [ ] **Animation library policy.** Default is **zero-dep** — Svelte's built-in `transition:` + the keyframes/helpers above cover ~95% of the surface. Escalate to **Motion One** (`motion.dev`, ~9 KB, hardware-accelerated, integrates cleanly with runes) *only* when one of three triggers shows up: (a) a feature genuinely needs FLIP layout transitions across screen boundaries, (b) a feature needs spring-physics-driven gestures (sheet drag-dismiss with velocity), (c) we need to animate across `document.startViewTransition` boundaries in a way Svelte's transitions can't. **GSAP and similar heavy libraries (>30 KB) are forbidden** — they blow the bundle budget (Phase 8.3) and signal we've defaulted to "more animation" instead of better animation.
- [ ] **Motion One integration recipe** (only run when a trigger above fires): `pnpm --filter @conusai/ui add motion`. Re-export from `packages/ui/src/lib/motion/index.ts` so all consumers go through one entry point:
  ```ts
  // packages/ui/src/lib/motion/index.ts
  export { animate, spring, timeline, scroll, inView } from 'motion';
  // Helper that reads spring physics from CSS tokens — keeps animation values inside the token system.
  export function tokenSpring(tokenName: '--spring-snappy' | '--spring-gentle' | '--spring-bouncy') {
    const v = getComputedStyle(document.documentElement).getPropertyValue(tokenName).trim().split(/\s+/).map(Number);
    return spring({ stiffness: v[0], damping: v[1], mass: v[2] });
  }
  ```
  All call sites use `tokenSpring('--spring-snappy')` rather than literal numbers — same discipline as `--ease-*` curves. `prefers-reduced-motion` short-circuits every animation to opacity-only inside this entry point so consumers can't forget.

### 2.4 Iconography
- [ ] Standardize on `lucide-svelte` (already in `apps/web`). Add to `packages/ui` as a peer and re-export a curated set via `packages/ui/components/icons/`.
- [ ] Strip ad-hoc inline SVGs; replace with `Icon` primitive that enforces stroke width 1.5, size tokens (`--icon-sm: 16`, `--icon-md: 20`, `--icon-lg: 24`).
- [ ] **Known direct-import violators to migrate first** (audit 2026-05-23): `apps/web/src/routes/account/+page.svelte`, `apps/web/src/routes/account/billing/+page.svelte`, `apps/web/src/routes/account/usage/+page.svelte`. After these three, the token-audit script (Phase 1.2) gates against new ones.

### 2.5 Svelte 5 runes migration (scheduled, not opportunistic)
- [ ] **Inventory:** auto-generated list of every `.svelte` file in `packages/ui` and `apps/*` that still uses `let`-reactivity, legacy stores, or `export let` props. Output to `docs/ui-runes-inventory.md` via `scripts/dump-runes-status.mjs`. Audit (2026-05-23) baseline: **`packages/ui/src/lib/components/` = 1 of 10 on runes** (only [`AppShell.svelte`](../packages/ui/src/lib/components/AppShell.svelte) uses `$props()`; the other 9 — `CapabilityCard`, `PlanBadge`, `PlanCard`, `QuotaBanner`, `ThemeProvider`, `ThemeSwitcher`, `ToastHost`, `UsageMeter`, `WorkspaceTree` — still use `export let`). **`packages/ui/src/lib/features/` = 0 of 11 on runes.** **`packages/ui/src/lib/stores/` = 8 of 8 on runes** (`.svelte.ts` files using `$state`). The `--max-warnings=N` floor in 2.5's ESLint ratchet should be derived by running `scripts/dump-runes-status.mjs` against this commit *first*, not guessed.
- [ ] **Hard deliverable for this phase:** every file in `packages/ui/src/lib/components/` (the primitives) is converted to runes (`$state`, `$derived`, `$effect`, `$props`) before Phase 3 starts. Primitives are the bedrock — they cannot ship with legacy reactivity if downstream code is rune-only.
- [ ] **Phase 3 deliverable:** every file in `packages/ui/src/lib/features/` converted.
- [ ] **Phase 4 deliverable:** every `.svelte` file in `apps/*` converted.
- [ ] CI gate: an ESLint config with a **global `'warn'` floor** + directory-scoped `'error'` `overrides` fails the build on `export let` / `$:` / `import { writable } from 'svelte/store'` outside `.svelte.ts` files. The global floor makes the full migration debt visible in every `pnpm lint` run from day one — without it, legacy violations in unmigrated dirs are silently invisible. Concrete shape in `eslint.config.js`:
  ```js
  // Global floor — visible in every pnpm lint run from day one of Phase 2.
  // Surfaces the migration debt without blocking CI until a directory closes.
  { files: ['**/*.svelte'],
    rules: { 'svelte/no-legacy-reactive-statement': 'warn', 'svelte/no-export-let': 'warn' } },
  // Phase 2 exit: primitives flip to error-level
  { files: ['packages/ui/src/lib/components/**/*.svelte'],
    rules: { 'svelte/no-legacy-reactive-statement': 'error', 'svelte/no-export-let': 'error' } },
  // Phase 3 exit: extend to features/
  { files: ['packages/ui/src/lib/features/**/*.svelte'],     rules: { /* same error rules */ } },
  // Phase 4 exit: extend to apps/
  { files: ['apps/**/*.svelte'],                              rules: { /* same error rules */ } },
  ```
  Each phase boundary adds one error-level block. The repo lives with mixed-mode warnings in unmigrated dirs and hard errors in migrated dirs — both visible in `pnpm lint` output, only the latter blocks CI.
- [ ] **Ratchet, don't bigbang:** the gate ships as a **warning** (`eslint --max-warnings=N` with N = current count at each phase) on first introduction, then flips to **error** at each phase boundary once that directory's migration deliverable lands. Matches the way the Svelte core team migrated SvelteKit itself and avoids one giant unreviewable PR.

### 2.6 Primitive gallery (moved earlier from Phase 8.4)
- [ ] Stand up a dev-only route `/_/ui` in `apps/web` that auto-discovers every `.svelte` file under `packages/ui/src/lib/components/` (Vite `import.meta.glob('/packages/ui/src/lib/components/**/*.svelte')`) and renders each with a small set of representative prop fixtures co-located as `Component.fixtures.ts`.
- [ ] Gate behind `import.meta.env.DEV` so it never ships to prod. No Storybook dependency — the goal is the gallery, not the framework around it; 200 lines of SvelteKit covers it.
- [ ] **Rule:** every new primitive that lands in Phase 2.2/2.3/2.4 ships with a fixtures file in the same PR. Visual review of primitives now happens in Phase 2, not Phase 8 — catches design drift while context is fresh.
- [ ] Mirror the route in `apps/browser-shell` so iOS-sim review of primitives in isolation is possible without navigating through real screens.

### 2.7 Cross-cutting primitive extraction sweep
- [ ] Build the **cross-cutting primitives** that ≥2 Phase 4 screens consume — *before* Phase 4 starts, so screen PRs are pure consumption with no API debates. Each primitive lands in its own PR with a fixtures file in `/_/ui`:
  - `Button.svelte` (consumed by 4.3 login, 4.4 account, 4.5 billing, 4.9 error)
  - `Field.svelte` (consumed by 4.3 login; designed to also cover settings forms)
  - `Chip.svelte` (consumed by 4.1 suggestion chips, 4.7 capability filters, extracted from existing `ContextChip` / `CapabilityPinChip` / `SuggestionChips`)
  - `EmptyState.svelte` (consumed by 4.9 error + every "no data" state across the app)
- [ ] **Screen-specific primitives stay in their Phase 4 sub-phase** but ship as a **primitive PR first, then screen PR**: `PageHeader` (4.4), `DataTable` + `StatusBadge` (4.5; the billing-specific `InvoiceStatusBadge` wrapper lands in the screen PR itself, not the primitive PR), `QuotaList` (4.6 → `features/`). This separates "is the API right?" from "does the screen look right?" — two reviews, two scopes.

**Exit criteria:** Token audit script returns 0 violations. All headings/display/labels via `Type`; body via semantic elements + token classes (per 2.2). All icons via `Icon`. Both themes pass token-parity test. Every primitive in `packages/ui/src/lib/components/` uses Svelte 5 runes. `/_/ui` route lists every primitive (including the 2.7 cross-cutting set) with at least one fixture.

**Visual audit gate (Principle #12):** Re-shoot the full baseline on web + iOS simulator after the token regeneration and `Type`/`Icon` swap. Diff expected within the **Principle #3 "full route" tier (0.5%)** — sub-pixel font-metric noise only. Any larger diff means a token's computed value changed — diff the generated `tokens.css` against the previous commit and reconcile before merging. Run both `paper` and `forge` themes on iOS to confirm `prefers-color-scheme` propagates through the Tauri WebView.

---

## Phase 3 — Shared App Shell (one shell, every surface)

**Goal:** A single `AppShell` that adapts to web (desktop), Tauri desktop, and mobile (iOS/Android) without forks.

### 3.1 Layout regions
- [ ] **Rewrite** `packages/ui/components/AppShell.svelte` against the slot contract below: named slots `topbar`, `rail` (sidebar), `main`, `composer`, `overlay`. The existing 930-byte / 51-line file is a working v0 (already on Svelte 5 runes, two-column flex, `--paper`/`--ink`/`--rule` token-bound, `sidebar` + `children` snippets) but has no breakpoints, no container queries, no `topbar`/`composer`/`overlay` slots, no landmark roles — treat its color-binding as informative reference and the rest as discardable.
- [ ] **First consumer:** [`apps/browser-shell/src/lib/mobile/MobileShell.svelte`](../apps/browser-shell/src/lib/mobile/MobileShell.svelte) — **621 lines** (audit 2026-05-23; original plan said 235 — corrected) of local CSS + composition logic, the single worst violator of the "≤ 20 lines per app layout" target. Migrating it to the new `AppShell` is the proof point that the slot contract works on mobile. **Scope reminder (Principle #7):** alongside `MobileShell.svelte`, the 7 helper files in `apps/browser-shell/src/lib/mobile/parts/` (inventoried in §0.1's disposition table) are absorbed by Phase 3.1–3.5 — some move to `packages/ui/`, some are folded into unified primitives, none stay app-local. Web migration follows once mobile lands.
- [ ] **WCAG 2.2 landmark roles (Principle #13):** each slot wraps its content in the correct landmark — `<header role="banner">` for `topbar`, `<nav role="navigation" aria-label="Workspace">` for `sidebar` when it's the primary nav (or `<aside role="complementary">` when it's secondary on screens where the topbar carries primary nav), `<main role="main">` for `main`, `<footer role="contentinfo">` for any footer slot, and **`<form aria-label="Message composer">`** for the composer slot — **NOT `role="search"`**, which is reserved for actual search/filter inputs (the `<SidebarSearch>` field is the legitimate `role="search"` consumer). Axe runs in Phase 1.4 will fail if any landmark is missing or duplicated. Verify in VoiceOver rotor — every landmark should be enumerable without entering content.
- [ ] **Sidebar role decision matrix + create `docs/ui-landmarks.md` stub** — codify which screens use which role so VoiceOver rotor stays consistent across the app. The matrix below ships as the initial content of `docs/ui-landmarks.md` *in this phase's PR* (not deferred to Phase 8.4 — auditors need a referenceable doc *during* Phase 3, not after). Phase 8.4 then expands the same file with every other landmark across the app.

  | Screen / route | Sidebar role | Reasoning |
  |---|---|---|
  | `/` (greeting / chat) | `navigation` + `aria-label="Workspace"` | Sidebar holds the primary nav (RECENT, Capabilities, Artifacts) |
  | `/account/**` | `navigation` + `aria-label="Workspace"` | Same sidebar, same primary-nav purpose |
  | `/login` | *(no sidebar)* | Sidebar slot empty; no landmark emitted |
  | Future: split-view screens with a secondary right panel | right panel = `complementary` | Left sidebar stays `navigation`; right panel is supplemental |

  The `AppShell` accepts a `sidebarRole?: 'navigation' | 'complementary'` prop (default `'navigation'`) so this stays a one-line opt-in at the call site, not a code branch inside the shell.
- [ ] Define three breakpoints:
  - **Compact** (`< 768px`): rail hidden, topbar with hamburger + brand + actions, composer fixed bottom, drawer slides over `main`.
  - **Medium** (`768–1023px`): rail collapsed (icons only, 64 px), expandable on hover/tap.
  - **Expanded** (`≥ 1024px`): rail full 260 px, persistent, topbar simplified.
- [ ] Implement via **named container queries** on `AppShell`, not viewport media — so the shell works inside Tauri windows of any size.
  ```css
  :global(.app-shell) { container-type: inline-size; container-name: app-shell; }
  @container app-shell (min-width: var(--bp-expanded)) { /* expanded */ }
  ```
  Breakpoint thresholds live in `tokens.css` as actual CSS custom properties — not just doc constants — so the container queries themselves stay inside the token system:
  ```css
  :root {
    --bp-compact:  768px;
    --bp-medium:  1024px;
    --bp-expanded: 1440px;
  }
  ```

### 3.2 Drawer / Sheet unification
- [ ] Extract generic `packages/ui/components/Drawer.svelte` + `Sheet.svelte` primitives backed by the native `<dialog>` element + focus trap + `inert` background. Current `packages/ui/components/AppDrawer.svelte` and `AppBottomSheet.svelte` (moved here in Phase 0.1) become thin wrappers around these primitives, then are deleted once `apps/browser-shell` consumes the primitives directly.
- [ ] Drawer = left/right edge slide (rail content on mobile). Sheet = bottom modal (attachment picker, capability detail). Both honor `--safe-*` insets and `prefers-reduced-motion`.
- [ ] **Explicit ARIA (Principle #13):** even though native `<dialog>` carries modal semantics implicitly, set `aria-modal="true"` and a required `aria-label` (or `aria-labelledby` referencing the sheet's title) on every instance. Survives screen-reader and polyfill quirks. Required-prop on the Svelte component — TypeScript fails the build if a consumer omits it.

### 3.3 App header
- [ ] Single `<AppHeader>` component at [`packages/ui/src/lib/components/AppHeader.svelte`](../packages/ui/src/lib/components/AppHeader.svelte) with three layout modes (`compact`, `medium`, `expanded`) auto-selected from container size. **One name, no aliases** (per Principle #13).
- [ ] Slots: `leading` (hamburger / back), `title` (workspace name + breadcrumb), `trailing` (chat icon, theme toggle, profile).
- [ ] On macOS Tauri: add `data-tauri-drag-region`, traffic-light inset via `padding-left: env(titlebar-area-inset-left)`.
- [ ] On iOS: respect `env(safe-area-inset-top)`.
- [ ] **Migration shim (Phase 0.1 → Phase 4 close):** the moved `AppTopBar.svelte` re-exports `AppHeader` with a `@deprecated` JSDoc tag; `ui:contracts` (§8.1) deletes it at the sunset deadline.

### 3.4 Sidebar
- [ ] Single `<Sidebar>` component at [`packages/ui/src/lib/components/Sidebar.svelte`](../packages/ui/src/lib/components/Sidebar.svelte) with sections via `<SidebarSection eyebrow="RECENT">`. Sections animate in with the cascade-in motion. **One name, no aliases** (per Principle #13).
- [ ] Density driven by a `--rail-density` CSS custom property (`compact` | `icons` | `expanded`) — *the `--rail-density` token name stays as an internal jargon scalar describing layout density semantics; the component on disk is `Sidebar`, the call site imports `Sidebar`*. Same component covers all three layouts without prop drilling, set by the parent `AppShell` container query.
- [ ] Search input pinned to top: `<SidebarSearch>` — sticky, blurred backdrop on scroll. This is the **legitimate `role="search"` consumer** (Principle #13).
- [ ] User footer: `<AccountMenuButton>` — avatar, name, plan badge; opens `<ProfileSheet>` on tap (mobile) or a dropdown menu (desktop).
- [ ] **Classification (Principle #10):** the unified `<WorkspaceTree>` moves to `packages/ui/src/lib/features/WorkspaceTree.svelte` (not `components/`) — "workspace" is domain language; consult the Principle #10 classification table. Collapses today's duplicated [`packages/ui/src/lib/components/WorkspaceTree.svelte`](../packages/ui/src/lib/components/WorkspaceTree.svelte) and [`packages/ui/src/lib/features/WorkspaceExplorer.svelte`](../packages/ui/src/lib/features/WorkspaceExplorer.svelte) into one file driven by container queries (no `compact|expanded` boolean prop). A future generic `<Tree items getLabel getIcon getChildren />` could live in `components/`, but until that abstraction exists the workspace-aware version stays in `features/`.
- [ ] **Migration shim:** the moved `Rail.svelte` (if Phase 0.1's inventory ends up needing one as a stop-gap) re-exports `Sidebar` with a `@deprecated` JSDoc tag; deleted at Phase 4 close via the `ui:contracts` gate.

### 3.5 Composer
- [ ] Promote `AgentChatComposer.svelte` to `packages/ui/components/Composer.svelte`, using `--r-md` outer / `--r-sm` inner send button (nested radius rule §5).
- [ ] States: rest, focus (ember ring), submitting (skeleton shimmer), disabled, error.
- [ ] Attachments row uses `<Chip>` primitives, draggable to reorder (desktop), long-press to remove (mobile).
- [ ] Slash-command popover anchored above the composer; keyboard navigable.

### 3.6 Android emulator smoke (do not defer to Phase 5)
- [ ] Stand up `tauri android dev` against the new `AppShell` on a Pixel 8 emulator at minimum (target API 34). Validate the layout assumptions baked into 3.1–3.5 *now*, not after eight more phases of code is written against them:
  - Soft keyboard inset behavior (Android Chromium WebView reports `visualViewport` differently than iOS Safari — composer must stay visible).
  - Hardware back button: closes drawer/sheet first, then history-back, then default-quit. Wire the listener in Phase 3.6, not Phase 5.2.
  - Status bar / nav bar safe-areas via `env(safe-area-inset-*)` — Android 14+ supports it, older versions don't; document the fallback.
  - Touch-target ≥ 48×48 dp (Material — slightly larger than iOS's 44 pt).
  - WebView text rendering for Geist — Chromium handles variable fonts differently than WebKit; expect a 0.3–0.5% diff on the same screen.
- [ ] Commit one screenshot per Phase 3 layout breakpoint from the Android emulator alongside the iOS ones. `scripts/cross-platform-diff.mjs` (Phase 1.3) gains an Android column.

**Exit criteria:** `apps/web/+layout.svelte` and `apps/browser-shell/+page.svelte` each shrink to ≤ 20 lines; both render the same `AppShell` with different slot content only. Android emulator passes **functional, layout, safe-area, keyboard, and touch-target gates** for every Phase 3 breakpoint — same correctness bar as iOS. **Android visual diff is enforced at the cross-platform perceptual threshold (Principle #3, 2% tier) with documented platform-chrome masks, not the desktop Chromium full-route 0.5% tier.** Chromium-on-Android handles variable fonts differently than WebKit; expecting desktop-strict pixel parity from a Tauri Android WebView is realism failure, not high standards.

**Visual audit gate (Principle #12 + #13):** Capture every breakpoint (`--bp-compact` 360/767, `--bp-medium` 1024, `--bp-expanded` 1440) on web **and** the corresponding device sizes on iOS sim (iPhone SE = compact, iPhone 16 Pro = compact, iPad mini sim = medium, iPad Pro sim = expanded). Verify: rail/Sidebar collapses at the right breakpoint, drawer slides from the **left edge** (HIG default for primary-nav drawers, never right unless explicit), safe-area insets render on iPhone notch + Dynamic Island, container query (`@container app-shell`) doesn't false-trigger when the iOS keyboard appears, composer stays above the keyboard with full send-button visibility (`visualViewport` listener verified). **Landmark check:** open VoiceOver rotor on iOS and the axe Landmarks panel on web — `banner`, `navigation` (or `complementary`), `main` must each appear exactly once per screen; no duplicates, no missing. `form[aria-label="Message composer"]` appears once on chat screens only. `search` (real search inputs like `<SidebarSearch>`) at most once per screen, never on the composer. Touch-target check: every tappable element ≥ 44×44 pt per HIG (use Playwright's `getByRole().boundingBox()` in an assertion). Manual walk: rotate iPhone sim portrait↔landscape — layout must remain stable, no scroll jump.

---

## Phase 4 — Screen-by-Screen Pixel Pass

For each screen below: (a) match the screenshot in [`docs/tasks/perfect-ui-task.md`](tasks/perfect-ui-task.md), (b) replace any local CSS with shared primitives, (c) add Playwright visual test, (d) tick the box.

> **PR cadence (decided):** **one PR per sub-phase** (`4.1` → its own PR, `4.2` → its own, …). Trivial polish sub-phases (`4.10` toasts/live-region) may be bundled with an **adjacent** sub-phase (e.g. 4.10 with 4.9) only if (a) the combined diff stays under 200 lines, and (b) the visual-audit gate passes cleanly with no waived diffs. "Combined" PRs still require separate audit artifacts per screen — bundling saves a PR, not an audit. Per-sub-phase keeps blast radius small and lets the audit gate (#12) run against a focused change set — auditing ten screens in one PR makes regressions invisible.

> **Extraction notes** — each subsection lists `**Extract from:**` the existing component(s) the work strangler-figs *from*. "MISSING" means build from scratch with no prior file to consume. **Placement** of each net-new file (`components/` vs `features/`) is called out per Principle #10.

### 4.1 Greeting / Empty Chat (`/`)
- **Extract from:** `apps/web/src/routes/+page.svelte` (134-line `<style>` block) + `packages/ui/src/lib/features/SuggestionChips.svelte`.
- [ ] Centered logo at `--s-7` top inset, `Type variant="display"` greeting ("Good evening, {firstName}.") at the display opsz axis.
- [ ] Composer centered, max-width `--composer-w`, never exceeds 92vw on mobile.
- [ ] Suggestion chips: 2-row wrap on mobile, single-row scroll on desktop; stagger animate 40 ms.
- [ ] Removes any test/dev banners on prod builds.

### 4.2 Active Chat
- **Extract from:** `packages/ui/src/lib/features/AgentChatStream.svelte` → split into `<MessageList>` + `<MessageBubble>` + `<ThinkingIndicator>` primitives. `<ToolCard>` extracts from existing `packages/ui/src/lib/features/ToolCallCard.svelte`. `<Composer>` extracts from `AgentChatComposer.svelte` (moved to `components/` in Phase 0.1, renamed here).
- [ ] Message list uses `<MessageList>` with intersection-observer auto-scroll lock + "jump to latest" pill when user scrolls away.
- [ ] `<MessageBubble role="user|assistant">` per [`docs/ui-design.md`](ui-design.md) §8.2 (ember left rail, asymmetric radii).
- [ ] `<ThinkingIndicator>` shows immediately on submit, removed on first token.
- [ ] `<ToolCard>` running/success/error states match §8.4.
- [ ] Sticky composer at bottom with deeper shadow on scroll (`box-shadow` swaps via `IntersectionObserver` sentinel).

### 4.3 Login (`/login`)
- **Extract from:** `apps/web/src/routes/login/+page.svelte` (local form markup). `<Field>` and `<Button>` are MISSING — build as new primitives in `packages/ui/components/`.
- [ ] Two-column on `≥1024`: left = Foundry poster (teal gradient + noise overlay), right = form. Single column on mobile, poster shrinks to 30vh header.
- [ ] Form uses `<Field>` primitive with floating label and `aria-describedby` for errors.
- [ ] Social/OIDC buttons use shared `<Button variant="ghost-outline">`.

### 4.4 Account (`/account`)
- **Extract from:** `apps/web/src/routes/account/+page.svelte` (168-line `<style>` block — largest in `apps/web`). `<PlanCard>` already in `packages/ui/src/lib/components/PlanCard.svelte`. `<PageHeader>` is MISSING — build new.
- [ ] Header: `<PageHeader eyebrow="ACCOUNT" title="..." subtitle="...">`.
- [ ] Two-column grid on `≥768`: profile card + plan card. Single column on mobile.
- [ ] `<PlanCard>` migrate to shared semantic tokens, ensure 44 px hit targets.

### 4.5 Billing (`/account/billing`)
- **Extract from:** `apps/web/src/routes/account/billing/+page.svelte`. `<DataTable>` and `<StatusBadge>` are MISSING — build new primitives in `packages/ui/src/lib/components/`. `<InvoiceStatusBadge>` (the billing-aware wrapper) is built in `packages/ui/src/lib/features/billing/` *in this sub-phase's screen PR*, not the primitive PR.
- [ ] Invoice table → `<DataTable>` primitive (sortable, sticky header, mobile cards layout under `768px`).
- [ ] Generic status badge: `<StatusBadge status="success|warning|danger|neutral" label="Paid">`, using `--color-success-soft` / `--color-warning-soft` / `--color-danger-soft` semantic tokens (per §2.1's `--color-*` canonical naming).
- [ ] Billing-specific wrapper: `<InvoiceStatusBadge status="paid|due|overdue">` in `features/billing/` — pure mapping (`paid → success`, `due → warning`, `overdue → danger`) over `<StatusBadge>`. The moment invoice-specific behavior arrives (retry URL, payment-provider state), it lives here, not in the primitive.

### 4.6 Usage (`/account/usage`)
- **Extract from:** `apps/web/src/routes/account/usage/+page.svelte` + existing `packages/ui/src/lib/components/UsageMeter.svelte` + `packages/ui/src/lib/features/CapabilityRow.svelte`. `<QuotaList>` is MISSING — build new at **`packages/ui/src/lib/features/QuotaList.svelte`** (per Principle #10: it composes `<CapabilityRow>` and reads app-state quotas, so `features/` not `components/`).
- [ ] `<UsageMeter>` upgrade: linear bar on desktop, radial on mobile (more glanceable). Drop any dual-implementation.
- [ ] Capability quotas listed via `<QuotaList>` using `<CapabilityRow>`.

### 4.7 Capabilities Browser
- **Extract from:** `packages/ui/src/lib/features/CapabilityBrowser.svelte` + `WorkspaceTree.svelte` (components) + `WorkspaceExplorer.svelte` (features) — collapse the latter two into a single `<WorkspaceTree>` at `packages/ui/src/lib/features/WorkspaceTree.svelte` (per Phase 3.4 classification — `features/`, not `components/`) driven by container queries. `<Chip>` is MISSING — extract from `ContextChip.svelte` + `CapabilityPinChip.svelte` + `SuggestionChips.svelte`. `stores/pins.ts` is MISSING.
- [ ] `<CapabilityBrowser>` becomes a two-pane on desktop (list left, detail right) and a single list + sheet on mobile.
- [ ] Filtering chip rail uses `<Chip>` primitives; pinned chips persist to localStorage via `stores/pins.ts`.

### 4.8 Artifacts
- **Extract from:** [`packages/ui/src/lib/features/screens/ArtifactsScreen.svelte`](../packages/ui/src/lib/features/screens/ArtifactsScreen.svelte) + [`packages/ui/src/lib/features/screens/ArtifactRow.svelte`](../packages/ui/src/lib/features/screens/ArtifactRow.svelte) — both exist (audit re-check 2026-05-23 found them under the `features/screens/` subdirectory, which the original audit missed). Work is collapsing any mobile/desktop forks and consuming the new `<Sheet>` primitive (Phase 3.2). **Sequencing:** §4.8 cannot start until Phase 3.2 (`<Sheet>` primitive) and Phase 2.7 (`<Chip>`, `<EmptyState>`) have landed; the existing files swap their old sheet-like wrappers for the canonical `<Sheet>`.
- [ ] **Subdirectory note (Principle #10):** the `features/screens/` subdirectory currently holds `ArtifactsScreen`, `ArtifactRow`, `ChatScreen`, `CapabilitiesScreen`, `CapabilityDetailSheet`, plus the helper `buildInvocationPrompt.ts`. The `screens/` subdirectory is fine — it groups multi-primitive composed screens under `features/`. Phase 4 sub-phases reference files at `features/screens/` where they live; do not flatten the subdirectory.
- [ ] `<ArtifactsScreen>`: confirm grid (3-col desktop / 2-col tablet / 1-col mobile) of `<ArtifactRow>` cards. Empty state via `<EmptyState>` (Phase 2.7 primitive).
- [ ] Preview opens in `<Sheet>` on mobile, side panel on desktop.
- [ ] **Data wiring** — verify the existing store/capability source still works after Phase 2.5's runes migration; no new store needed.

### 4.9 Error & Empty states
- **Extract from:** `apps/web/src/routes/+error.svelte` (36-line `<style>` block). `<EmptyState>` is MISSING — build new primitive.
- [ ] `+error.svelte` → centered `<EmptyState icon kind title body action>`.
- [ ] Add empty states for: no chats, no artifacts, no capabilities, no invoices. Each illustrated with a single hairline-rule SVG (one accent stroke in `--ember`).

### 4.10 Toasts & Live region
- **Extract from:** existing `packages/ui/src/lib/components/ToastHost.svelte` + `packages/ui/src/lib/utils/LiveAnnouncer.svelte` — both already exist; this is an audit-and-polish step.
- [ ] Audit `ToastHost.svelte` — ensure top-right on desktop, top-center on mobile under topbar, `--safe-area-inset-top` aware.
- [ ] `LiveAnnouncer` confirmed wired for all async actions (send message, capability invoked, error).

**Exit criteria:** Every screen passes visual diff at the **Principle #3 "full route" tier (0.5%)**, axe 0 violations, and uses zero local CSS. Primitive isolation shots in `/_/ui` keep the 0.1% tier — distinct gate, distinct number.

**Visual audit gate (Principle #12 + #13):** Per **sub-phase** (4.1 → 4.10), each screen ships in its own PR and goes through web + iOS simulator audit before merge. Per-screen checks:
   - **Landmarks (#13):** axe Landmarks panel + iOS VoiceOver rotor — every screen has `banner` + `main` exactly once; nav/complementary as appropriate; `form[aria-label="Message composer"]` on chat screens only; `role="search"` only where a real search/filter input renders (e.g. `<SidebarSearch>`), **never on the composer**.
   - **Position & layout:** greeting (4.1) composer centered with `max-width: var(--composer-w)`, never overflows on 360px; chat (4.2) messages scroll above a fixed composer with no overlap at any keyboard state; login (4.3) two-column reflows to single column at `--bp-compact`; account/billing/usage (4.4–4.6) header → grid → cards stack vertically on mobile with 16px gutters minimum.
   - **iOS-specific:** safe-area top/bottom insets visible (no clipped content under notch / home indicator), swipe-back doesn't conflict with horizontal scrollers in 4.5's `<DataTable>`, keyboard inset on composer doesn't cover send button, sheet/drawer use native-feel ease curves, `forge` theme flip respects `prefers-color-scheme`.
   - **Touch targets:** every interactive element ≥ 44×44 pt (HIG) — Playwright assertion per screen.
   - Attach side-by-side web/iOS screenshots per screen + axe report + landmark map to the PR description.

---

## Phase 5 — Mobile / Tauri Native Polish

**Goal:** The browser-shell looks native on iOS and Android, identical chrome on macOS / Windows Tauri.

> **Scope clarification:** **Android shell smoke happens in Phase 3.6** (keyboard, back-button, safe-areas, WebView font rendering) — *not* deferred to here. Phase 5 is the **native-polish layer** on top of an already-validated shell: haptics, swipe-back, status-bar styling, window chrome on desktop, deep iOS-vs-Android divergence handling. Execution order within Phase 5 stays web + Tauri desktop → iOS → Android because the **Android NDK / Gradle / emulator toolchain** is the heaviest to set up — but the shell behavior on Android was already proven in 3.6, so this is incremental polish, not the first encounter with the platform.

### 5.1 Platform detection
- [ ] [`packages/ui/src/lib/utils/platform.ts`](../packages/ui/src/lib/utils/platform.ts) exports **capability-precise** detectors (per Principle #15) — pure functions, evaluated once at module load:
  ```ts
  export function getPlatform(): 'web' | 'ios' | 'android' | 'macos' | 'windows' | 'linux';
  export function isTauriRuntime(): boolean;      // running inside any Tauri WebView
  export function isIOSWebView(): boolean;        // iOS Safari OR Tauri iOS WebView
  export function isAndroidWebView(): boolean;    // Chromium Android OR Tauri Android WebView
  export function isMacOSDesktop(): boolean;      // Tauri on macOS, NOT the iOS sim
  export function isWindowsDesktop(): boolean;    // Tauri on Windows
  export function supportsHaptics(): boolean;     // Tauri haptics plugin available OR navigator.vibrate
  export function supportsSafeAreaEnv(): boolean; // env(safe-area-inset-*) returns non-zero on test surface
  export function supportsViewTransitions(): boolean; // document.startViewTransition exists
  ```
  Avoid vague identity booleans like `isWeb` — everything is web in a Tauri WebView; the right question is *what can this runtime do*. Code that wants "non-Tauri browser" writes `!isTauriRuntime()`; code that wants "iOS-specific styling" writes `isIOSWebView()` and gets both Safari and Tauri iOS without an OR.
- [ ] `data-platform` attribute set on `<html>` in `app.html` of each app via tiny pre-hydration script (`ThemeScript.ts` pattern) — value is the `getPlatform()` return.

### 5.2 Safe areas & gestures
- [ ] All fixed/sticky elements consume `env(safe-area-inset-*)` via `--safe-top`, `--safe-bottom`, `--safe-left`, `--safe-right` tokens.
- [ ] Surface the safe-area values on `<html>` via the `ThemeScript` pattern (runs pre-hydration so first paint is correct — no jump on iOS).
- [ ] iOS swipe-back: enable via Tauri webview config; ensure routes opt-in/opt-out via `<svelte:head data-allow-swipe-back>`.
- [ ] Android back button: trap in shell → close drawer/sheet first, then history back.

### 5.3 Haptics
- [ ] `packages/ui/utils/haptics.ts`: light tap on send, success tick on capability completion, warning on error. Calls `@tauri-apps/plugin-haptics` when present; falls back to `navigator.vibrate()` on web/Android browsers; no-op when neither is available. Single API, three backends.
- [ ] **Tauri capability wiring:** `pnpm add` alone is not enough. Add `haptics:default` to `apps/browser-shell/src-tauri/capabilities/main.json` (or the relevant capability file) so the webview is actually permitted to invoke the plugin. Add the corresponding Cargo dependency in `apps/browser-shell/src-tauri/Cargo.toml`. Without these two steps the plugin call will silently fail at runtime — verify with a manual tap on iOS sim before ticking this box.
- [ ] **Runtime verification:** launch the iOS simulator via `pnpm --filter browser-shell tauri ios dev`, fire a haptic from the composer's send button, and confirm (a) the simulator vibrates the device, (b) no `permission denied` or `plugin not registered` warning appears in the Tauri console. Same drill on Android via `tauri android dev` and in `tauri dev` on macOS (no-op fallback path).

### 5.4 Keyboard handling (mobile)
- [ ] Composer auto-resizes above the on-screen keyboard using `visualViewport` listener — already needed; codify in `<Composer>`.
- [ ] iOS: disable input zoom via `font-size: max(16px, var(--t-body))`.

### 5.5 Theming respect
- [ ] Honor `prefers-color-scheme` on first load, but persist user override in `localStorage`. Tauri: sync with system appearance via `@tauri-apps/api/window`.
- [ ] `<ThemeSwitcher>` becomes a three-way toggle (system / paper / forge) and lives in the `<AccountMenuButton>` menu.

### 5.6 Window chrome (desktop Tauri)
- [ ] macOS: hidden titlebar, traffic-light inset via `padding-left: env(titlebar-area-inset-left)`, drag region on topbar.
- [ ] Windows: custom min/max/close buttons rendered in the topbar trailing slot, only when `isWindows && isTauri`.

**Exit criteria:** Manual run-through on iPhone simulator, Android emulator, macOS desktop, Windows desktop — every screen matches the web equivalent and the screenshots in `perfect-ui-task.md`.

**Visual audit gate (Principle #12):** This phase *is* the iOS-heavy audit — promote it from gate to deliverable. For every screen touched, record a short screen-capture (≤ 10s) on iPhone 16 Pro sim covering: launch → screen → primary action → secondary action → back. Attach to PR. Verify haptics fire on real interactions (not just on mount), bottom-sheet drag-dismiss snaps correctly, pull-to-refresh doesn't fight the chat scroll, status-bar style adapts to theme. Web audit still runs for parity — anywhere iOS and web diverge visually beyond the documented platform-chrome allowlist, fix iOS, not web.

---

## Phase 6 — Micro-Interactions & Motion Polish

**Goal:** Every interaction has a deliberate, restrained motion. No spinners, no jank. Every bullet below is tagged with its Principle #14 purpose — `[feedback]` / `[continuity]` / `[hierarchy]` / `[delight]`. An animation that can't be tagged doesn't ship.

- [ ] **`[hierarchy]` Page-load cascade** per [`docs/ui-design.md`](ui-design.md) §6 exactly — logo 80 ms, rail sections 160–320 ms, user chip 360 ms, greeting 420 ms, composer 560 ms, chips 680–920 ms. Total ≤ 920 ms (well under the 3 s task budget). Uses `--ease-emphasized-decelerate` for each entering element.
- [ ] **`[feedback]` Chat send:** composer scale `0.93` rebound (~180 ms, `--spring-snappy`) + message slide in from right (`msg-in-user`, ~220 ms, `--ease-emphasized-decelerate`). Send button never spins — the rebound *is* the acknowledgement.
- [ ] **`[continuity]` Streaming:** AI left rail traveling-ember gradient (loop, ~1.4 s/cycle, `--ease-linear` — opacity only, no positional linear) + cursor pulse on last char (1.0 s/cycle). Indicates "still working" without false-precision progress bars.
- [ ] **`[feedback]` Tool card transitions:** running → success/error radial flash (~280 ms, `--ease-emphasized-decelerate` — entering element settles into place), then settle. The flash is the result acknowledgement; the settle restores reading calm.
- [ ] **`[continuity]` View transitions:** use the View Transitions API (`document.startViewTransition`) for in-app navigation; fallback to `view-fade-in` (~200 ms, `--ease-standard`). Preserves spatial context — rail item → screen morph rather than blank → repaint.
- [ ] **`[feedback]` Hover lifts:** chip / button lift `translateY(-1px)` + ember border on pointer-fine devices only (`@media (hover: hover) and (pointer: fine)`). ~120 ms, `--ease-standard`. Mobile gets no hover — taps get the press-rebound instead.
- [ ] **`[delight]` (rare, reserved):** success milestones — first capability invoked, first artifact saved, plan upgraded. `--spring-bouncy` badge appearance, ≤ 320 ms, fires at most once per session per milestone. Anything more frequent moves to `[feedback]` and loses the bounce.
- [ ] **Reduced-motion gate:** every animation above clamps to an 80 ms opacity cross-fade under `prefers-reduced-motion: reduce`. Verified by `apps/web/e2e/visual/reduced-motion.spec.ts` — toggles the media query, asserts no `transform`/`translate` animations run on the canonical **top-5 task paths defined in Phase 1.5** (imported from `e2e/fixtures/task-paths.ts`), and re-shoots the visual baseline (must match the static baseline within `< 0.1%`).
- [ ] **Per-task duration audit (coarse signal, not perceived-duration measurement):** add `apps/web/e2e/motion-budget.spec.ts` that walks each top-5 task path (Phase 1.5), reads computed `animation-duration` + `transition-duration` on every element on the active path with `page.evaluate`, and asserts the total `≤ 3000 ms`. Honest framing: a 3000 ms *sum* doesn't equal a 3000 ms *perceived* duration (parallel animations are nearly free) — but it catches the specific regression of "death by a thousand small animations" that's invisible per-PR but disastrous cumulatively. Coarse signal, real value.
- [ ] **Per-transition rule (sharper signal, complements the budget):** no single `transition-duration` or `animation-duration` exceeds **400 ms** except the page-load cascade (which is `[hierarchy]`-tagged and intentional). Enforced by `scripts/check-motion-durations.mjs` — greps `tokens.css` and any `transition:` / `animation:` declarations across the repo, fails CI on any value > 400 ms outside the cascade allowlist. Catches per-animation bloat the sum-based budget can hide.
- [ ] **Chained-animation rule:** no animation chains (`animation: a 200ms, b 200ms 200ms`) after user input unless each step demonstrably aids task comprehension (loading → success → settle for a tool card counts; sequential decorative reveals do not). Reviewer judgment, no automated check — but cite this rule in PR review whenever a chained animation appears.
- [ ] **`scripts/check-motion-purpose.mjs` — automate the purpose-tag rule, don't leave it to reviewer folklore.** Greps the repo for any of: a `transition:` declaration in CSS/Svelte, an `animation:` declaration, a call to `animate(` or `spring(` or `tokenSpring(`, or a `[transition|use:|in:|out:]` Svelte directive. For each hit, fails CI unless one of the following sits within 5 lines: a `data-motion-purpose="feedback|continuity|hierarchy|delight"` attribute on the same element, a sibling comment matching `/\*\s*\[(feedback|continuity|hierarchy|delight)\]/` on the keyframe/transition definition, or the file is in the approved-helpers allowlist (`packages/ui/src/lib/motion/**`, where the helpers themselves carry the canonical tags). Activates at Phase 6 close as a warning, flips to error at Phase 7 close. Without this, the purpose-tag rule is a noble principle quietly murdered by deadlines — every motion rule is, the moment its enforcement is "reviewer grep."
- [ ] **Wire into `pnpm test:visual`** (Phase 8.1) so both the budget assertion *and* the per-transition rule run on every PR after Phase 6 lands. Two signals catch different failure modes; one signal alone is gameable.

**Visual audit gate (Principle #12 + #14):** Re-shoot baseline at full motion *and* at `prefers-reduced-motion: reduce` — both committed. Manually walk each top-5 task path on iOS sim; if anything feels sluggish, run the budget audit and find which animation grew. Every animation present in the PR must have its purpose tag visible in code (a `data-motion-purpose="feedback"` attribute or a comment on the keyframe definition) — reviewers grep for missing tags.

---

## Phase 7 — Accessibility & Internationalization

- [ ] **Focus:** unified `--focus-ring: 0 0 0 3px var(--ember-soft)` on every focusable; never remove outlines without replacement.
- [ ] **Skip links:** "Skip to main", "Skip to composer" injected in `AppShell`.
- [ ] **ARIA:** `<MessageList role="log" aria-live="polite">`, `<ToolCard role="status">`, dialogs use `<dialog>` + `aria-labelledby`.
- [ ] **Keyboard parity:** every mouse action reproducible via keyboard. Cmd/Ctrl+K opens command palette; `/` focuses composer; `Esc` closes any sheet/drawer. **Enforced by `apps/web/e2e/keyboard.spec.ts`** — explicit Playwright keyboard scripts asserting: Tab order walks landmarks in source order (banner → nav → main → contentinfo), Shift+Tab walks them in reverse, `/` focuses composer from any non-input element, Cmd/Ctrl+K opens the command palette and Esc closes it, Esc closes any open drawer/sheet without scroll position loss. **axe doesn't prove keyboard UX** — it checks ARIA + contrast; only real keyboard scripts catch focus traps, lost focus on dialog dismiss, and tab-order regressions.
- [ ] **Contrast:** automated WCAG 2.2 AA check on both themes (already in axe sweep).
- [ ] **i18n:** wrap all user-visible strings in `t('key')` from `packages/ui/utils/i18n.ts` (skeleton already in repo, populate). RTL: mirror layout via `dir="rtl"` honoring `--rail` on the right.

**Visual audit gate (Principle #12, Phases 6 + 7):** Re-shoot the full baseline on web + iOS with (a) `prefers-reduced-motion: reduce` forced, (b) `prefers-color-scheme: dark`, (c) `dir="rtl"` enabled, (d) 200% browser zoom / iOS Dynamic Type XXL. Each variant gets its own committed baseline directory. axe-core run on each variant. Keyboard-only walk-through recorded on web; VoiceOver walk-through recorded on iOS sim for the **top-5 task paths defined in Phase 1.5** (single canonical list — no drift between Phase 6 budget audit and Phase 7 a11y walks).

---

## Phase 8 — QA, Verification, Sign-off

### 8.1 Automated gates (CI must enforce)
- [ ] `pnpm lint` (svelte-check + biome + token audit script).
- [ ] `pnpm test` (vitest unit).
- [ ] `pnpm test:e2e` (Playwright on web).
- [ ] `pnpm test:visual` (visual diff against committed baseline at the **Principle #3 tier per surface**: primitives `0.001`, full routes `0.005`, cross-platform `0.02`, mechanical PRs `0.0001`; runs in Playwright Docker image; **bundles the motion-budget assertion + per-transition duration check from Phase 6** so motion gates run on every PR after Phase 6 lands).
- [ ] `pnpm test:a11y` (axe; 0 violations).
- [ ] `pnpm test:tauri` (WDIO on desktop + iOS sim).
- [ ] `pnpm size` (bundle budget via `rollup-plugin-visualizer` + size-limit; fails if initial gzipped JS > 180 KB or any single chunk > 80 KB). **Activates per-PR from Phase 2.2 onwards** — when the first primitive lands. Don't wait until Phase 8 to discover the budget was blown at Phase 4.
- [ ] `svelte-check --fail-on-warnings` across the whole monorepo — enabled once Phase 4 of the runes migration completes (apps converted). Until then it runs scoped to already-migrated directories per Phase 2.5's ratchet.
- [ ] `pnpm test:exports` — contract tests for `@conusai/ui`: a vitest suite that imports every entry the `package.json` exports map advertises *in the exact shapes consumers actually write*, and asserts each resolves to a defined value. Cover at minimum:
  **Base suite** (active from Phase 0.2):
  ```ts
  import Button from '@conusai/ui/components/Button.svelte';
  import AppShell from '@conusai/ui/components/AppShell.svelte';
  import { AppHeader, Sidebar } from '@conusai/ui/components';    // canonical names (Principle #13 — no dual aliases)
  import '@conusai/ui/tokens.css';
  import '@conusai/ui/foundry.css';
  import { isIOS, isTauri } from '@conusai/ui/utils/platform';
  ```
  **Motion-One suite** (split file `test:exports:motion`, gated behind `process.env.MOTION_ONE === '1'` — activates only after a Phase 2.3 escalation trigger lands Motion One in deps; before that, the import doesn't exist and would false-fail):
  ```ts
  import { animate, tokenSpring } from '@conusai/ui/motion';
  ```
  Naive map glob (`./components/*: ./src/lib/components/*`) works for `.svelte` files only if consumers include the extension — the contract test proves that's still true after every map change. Catches the failure mode where the map references a moved/renamed file and consumers break at runtime instead of build-time.
- [ ] `scripts/check-no-local-components.mjs` — fails CI when any new `.svelte` file lands under `apps/*/src/lib/components/**` or as a `<style>`-heavy component in `apps/*/src/routes/**` (>50 LOC). Anti-pattern "new components added to `apps/*` instead of `packages/ui`" was previously code-review-only; humans miss it. Activates in Phase 0.
- [ ] `scripts/check-motion-purpose.mjs` — enforces the Principle #14 purpose-tagging rule (see Phase 6 for spec). Warning at Phase 6 close, error at Phase 7 close. Without it, the rule becomes reviewer folklore.
- [ ] **`pnpm ui:contracts`** — a single script (`scripts/check-ui-contracts.mjs`) that bundles the architectural rules that lint can't easily express. More valuable than another screenshot of the login page. Fails CI on any of:
  1. **No forbidden imports from `apps/*` into `packages/ui`** — primitives can't reach back into consumer code.
  2. **No app-local component imports from `apps/browser-shell/src/lib/mobile/parts/**` after Phase 3 close** — the 7 files from §0.1's disposition table must be gone by then.
  3. **No raw color/radius/`px`-size literals outside token files** (overlaps with the §1.2 token-audit but bundled here for one-stop summary).
  4. **No `components/` file importing from `stores/` or `capabilities/`** — primitives are props-in / callbacks-out; the moment a primitive reads a store, reclassify it to `features/` (Principle #10).
  5. **No `features/` file importing from `apps/*/src/routes/`** — features compose primitives, routes compose features, never the reverse.
  6. **No `@deprecated` `App*` shim files remain after the Phase 4 close deadline** (per Principle #13 + §0.1 sunset rule). Before the deadline: warn. After: error.
  7. **No `role="search"` on a chat composer** — grep for `role="search"` in `Composer.svelte` and any descendant; fail if present (Principle #13).
  8. **No `variant="ember"` / `variant="forge"` / brand names in component prop values** — grep `apps/*` and `packages/ui/**/*.svelte` for the brand-name regex; fail if used as a prop value (Principle #15).

  Activates per-rule: rules 1, 3, 4, 5, 7, 8 from **Phase 0 close**; rule 2 from **Phase 3 close**; rule 6 flips warn→error at **Phase 4 close**.

### 8.2 Manual sign-off matrix
Run through every screen on:
- [ ] Safari iOS 18 (iPhone 16 Pro, iPhone SE)
- [ ] Chrome Android (Pixel 8)
- [ ] Safari macOS (Tauri shell + web)
- [ ] Chrome Windows (Tauri shell + web)
- [ ] Firefox Linux (web only)

For each, verify against the two screenshots in `perfect-ui-task.md`: desktop sidebar layout and mobile drawer.

**Final visual audit (Principle #12, ship gate):** Aggregate every per-phase audit artifact (Phases 0–7) into a single review document — full baseline set on web + iOS, all variant baselines (reduced-motion / dark / RTL / Dynamic-Type XXL), all VoiceOver + keyboard recordings. Sign-off requires a clean run of the **full** Playwright visual + axe suite on both surfaces *on the release candidate commit*, not just the cumulative per-phase results. Any regression introduced after a phase's gate passed blocks the release.

### 8.3 Performance budget
- [ ] LCP < 1.5 s on mobile 4G simulated.
- [ ] CLS < 0.02.
- [ ] **JS bundle (web, gzipped):** target ≤ 180 KB initial, but **ratchet from today's baseline** — don't hard-fail CI if the current shipped bundle is already above 180 KB. Phase 1.1 records today's number as `bundle-baseline.json`; from Phase 2.2 onwards CI fails any PR that *increases* the number (strict regression gate). Each phase aims to *reduce* toward 180 KB; the absolute target becomes a hard gate only at Phase 8.3 sign-off. A budget set above current reality is a motivational poster with a red X — useless.
- [ ] First Contentful Paint < 1.0 s.

### 8.4 Documentation
- [ ] Update [`docs/ui-design.md`](ui-design.md) with any new tokens / patterns introduced.
- [ ] Confirm the `/_/ui` primitive gallery (built in Phase 2.6) lists every primitive that landed in later phases, each with at least one fixture. CI: `pnpm --filter web dev` → curl `/_/ui` → assert one entry per `.svelte` file under `packages/ui/src/lib/components/`.
- [ ] Update [`README.md`](../README.md) with the "where to add UI" decision tree (always `packages/ui` first; `components/` vs `features/` per Principle #10; naming aliases per Principle #13).
- [ ] Add `docs/ui-landmarks.md`: one-line description of every WCAG landmark used in the app, where it's rendered, and the axe rule that enforces it. Reference table for new contributors.

---

## Tracking & Cadence

- **Branch per phase:** `ui/phase-N-<slug>`. Each phase merges only when its exit criteria pass.
- **Daily visual diff report** posted in the PR — required to merge **only for PRs touching rendered UI or visual baselines**. Skipped for Phase 0, package-only, doc-only, exports-map, and `audit-exempt:doc-or-rename`-labelled PRs (the report would only contain noise for them).
- **Definition of Done** for any UI change going forward:
  1. Uses only shared primitives from `packages/ui`.
  2. Uses only semantic tokens; token audit passes.
  3. Has a visual test + an a11y assertion.
  4. Matches reference within Principle #3 thresholds: **0.5% full-route** on web and Tauri desktop (Dockerized Chromium); **0.5% on iOS** (WebKit, stable across runs); **2% cross-platform perceptual on Android** (Chromium WebView, with documented platform-chrome masks). Functional/layout/keyboard/touch-target gates are equally strict everywhere; only the *visual diff threshold* relaxes for Android, because Chromium-Android font rendering noise routinely exceeds 0.5% even on byte-identical pages. "Identical" pixels across WebKit + Chromium + Tauri WebView don't exist; "within tier-appropriate threshold + masked chrome" does.
  5. Honors `prefers-reduced-motion` and `prefers-color-scheme`.

---

## Anti-patterns to delete on sight

Each entry shows its **activation point** — the phase after which it becomes a CI-enforced rule. Until then, it's a code-review hint only; flagging dozens of pre-existing violations as "delete on sight" before the migration path exists wastes review cycles.

- **`<style>` block in `apps/web/src/routes` or `apps/browser-shell/src/lib` containing color, font-size, or radius values** — activation: **Phase 1.2** (token-audit script flags), hard CI fail: **Phase 4** (after every screen extraction).
- **`lucide-svelte` imported directly outside `packages/ui/src/lib/components/icons`** — activation: **Phase 2.4** (after `Icon` primitive + curated re-exports land + the three named violators in `apps/web/src/routes/account/` are migrated).
- **New components added to `apps/*` instead of `packages/ui`** — activation: **Phase 0** via `scripts/check-no-local-components.mjs` (CI-enforced, see Phase 8.1). Prior version of this plan made it code-review-only; humans are famously bad linters at this kind of rule, so it's now automated from day one.
- **Hard-coded breakpoints (`@media (max-width: 768px)`)** — activation: **Phase 3.1** (after `--bp-*` tokens and `AppShell` container queries exist as the alternative).
- **Inline `transition:` style strings** — activation: **Phase 2.3** (after motion helpers `fadeRise` / `slideFromRight` / `cascade()` are exported).
- **Duplicate "mobile" vs "desktop" component pairs** — activation: **Phase 3.4** (after `--rail-density` + container queries prove the unified pattern works for `WorkspaceTree`).
- **Components without WCAG landmark roles where one applies** (per Principle #13) — activation: **Phase 3.1** (after `AppShell` slots model the pattern).
- **`<dialog>`-based primitives missing `aria-modal` + `aria-label`** — activation: **Phase 3.2** (TypeScript-required prop on `Drawer` / `Sheet`).
- **`export let` / legacy stores / `$:` in directories the runes ratchet has closed** — activation: **per-directory, Phase 2.5's ratchet schedule** (primitives @ end of Phase 2, features @ end of Phase 3, apps @ end of Phase 4).
- **Animations without a Principle #14 purpose tag** (`[feedback]` / `[continuity]` / `[hierarchy]` / `[delight]`) — activation: **Phase 6** (after the purpose-tagging pattern lands; reviewers grep for `data-motion-purpose=` or matching keyframe comments and block PRs missing them).
- **Animation libraries > 30 KB minified+gzipped** (GSAP, anime.js, lottie-web at full weight) — activation: **Phase 2.3** (after the Motion One escalation policy lands; bundle-size CI gate from Phase 8.3 catches them automatically once active per-PR).
- **Permanent dual public names for the same component** (`Rail` *and* `Sidebar` both exported; `TopBar` *and* `Header` both exported) — activation: **Phase 3** (after the canonical names land per Principle #13). Migration `@deprecated` shims are allowed until **Phase 4 close**, then the `ui:contracts` rule #6 in §8.1 deletes them.
- **`role="search"` on the chat composer** — activation: **Phase 3.1** (the moment `AppShell`'s composer slot lands). Enforced by `ui:contracts` rule #7. The composer is `<form aria-label="Message composer">`, not a search input.
- **Brand vocabulary in component or prop names** (`variant="ember"`, `<RailUserChip>`, `<ForgeButton>`) — activation: **Phase 2** (token rename in §2.1 establishes brand names belong only in tokens). Enforced by `ui:contracts` rule #8.
- **`createEventDispatcher` / `on:event` event-dispatch in new components** — activation: **Phase 2.5** (after the runes ratchet closes on `components/`). Callback props (`onSelect`, `onDismiss`, …) are the convention per Principle #15.
- **Vague identity booleans on the platform util** (`isWeb`, `isDesktop` with no specificity) — activation: **Phase 5.1** (after the capability-precise API lands). Use `isTauriRuntime()`, `isIOSWebView()`, `supportsHaptics()`, etc.

---

## Execution order (the dependency graph, not just the phase numbering)

Phase numbering is reading order, not execution order — some Phase 2 sub-items depend on Phase 3 (primitives need a shell), and **baseline must precede every refactor** (you can't measure what you've already changed). The graph below is the actual order; phase numbers are pointers into the body of the doc.

1. **Phase 0 (entire phase)** — mechanical reconcile PR. Moves chrome primitives, adds exports map, dedupes tokens, picks typography. No UI change. Hidden contradictions removed before they compound.
2. **Phase 1.1 → 1.5 (entire phase)** — **baseline first, always.** Inventory + token audit script + visual regression baseline + axe/Lighthouse + canonical top-5 task paths. *Nothing else moves until the "before" picture is committed.* If you regenerate tokens before this, your baseline includes the regeneration drift and the whole gate becomes meaningless.
3. **Phase 2.1 tokens + 2.5 runes inventory** *only*. Tokens-as-code (`tokens.json` → `tokens.css`), semantic aliases, runes ratchet warning floor in ESLint. Don't build primitives yet — the shell determines half their API.
4. **Phase 3.1–3.4** — build `AppShell`, `Drawer`/`Sheet`, `AppHeader`, `Sidebar` with named container queries + `--rail-density` (internal token name; component on disk is `Sidebar`). `MobileShell.svelte` is the first consumer; mobile proves the slot contract. **Android emulator smoke runs here**, not in Phase 5 — WebView keyboard/back-button differences invalidate layout assumptions late if you don't catch them now.
5. **Phase 2.2 + 2.3 + 2.4 + 2.6 + 2.7** — typography (`<Type>`), motion primitives (named curves + springs + Motion One escalation policy), iconography (`<Icon>`), gallery (`/_/ui`), and the **cross-cutting primitive extraction sweep** (`Button`, `Field`, `Chip`, `EmptyState`). These exist now because the shell exists.
6. **Phase 4 screens** — pixel-pass each screen using primitives that are already settled. Screen PRs are pure consumption, not "let me redesign the Button API while I'm here."
7. **Phase 5 native polish** — platform detection, safe-area tokens, haptics, keyboard, window chrome. Android already smoke-tested in step 4; this is the deep polish.
8. **Phase 6 + 7** — motion polish + a11y/i18n hardening.
9. **Phase 8** — release gate. Aggregate every per-phase artifact; full RC-commit re-run.

Two rules that bite if violated: **(a) baseline before refactor** (step 2 before step 3 — non-negotiable), **(b) shell before primitives** (step 4 before step 5 — primitives without a shell to live in get their APIs wrong).
