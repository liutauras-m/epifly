# UI Token Refactoring Plan — Epifly Brand Migration

**Scope:** Remove all legacy editorial tokens (Fraunces/Switzer/JetBrains, teal ember, cream paper) and replace with the Epifly brand system (Geist, orange/cyan, charcoal/off-white) across `packages/ui`, `apps/web`, and `apps/browser-shell`.

**Status:** Complete — executed 2026-05-11.

---

## 1. Epifly Design Tokens (target state)

### Colors

| Token | Light | Dark | Role |
|---|---|---|---|
| `--ink` | `#111111` | `#F8F8F8` | Primary text |
| `--ink-2` | `#3A3A3A` | `#C0C0C0` | Secondary text |
| `--ink-3` | `#767676` | `#888888` | Tertiary / muted text |
| `--paper` | `#F8F8F8` | `#111111` | Page background |
| `--paper-2` | `#F0F0F0` | `#1A1A1A` | Sidebar / elevated surface |
| `--paper-3` | `#E8E8E8` | `#222222` | Hover / active surface |
| `--rule` | `#E0E0E0` | `#2A2A2A` | Dividers, borders |
| `--seam` | `#C8C8C8` | `#3A3A3A` | Subtle hairline rules |
| `--ember` | `#FF6200` | `#FF6200` | Primary brand orange (was teal) |
| `--ember-2` | `#E05500` | `#FF7A20` | Darker/lighter orange variant |
| `--ember-soft` | `rgba(255,98,0,0.10)` | `rgba(255,98,0,0.12)` | Orange tint fills |
| `--ember-glow` | `rgba(255,98,0,0.22)` | `rgba(255,98,0,0.28)` | Orange glow/selection |
| `--cyan` | `#00D4FF` | `#00D4FF` | Secondary accent (new) |
| `--cyan-soft` | `rgba(0,212,255,0.10)` | `rgba(0,212,255,0.12)` | Cyan tint fills (new) |
| `--success` | `#1a7f4b` | `#22a060` | Semantic success |
| `--success-soft` | `rgba(26,127,75,0.13)` | `rgba(26,127,75,0.15)` | |
| `--danger` | `#b32400` | `#e03000` | Semantic error |
| `--danger-soft` | `rgba(179,36,0,0.13)` | `rgba(179,36,0,0.15)` | |
| `--shadow-sm` | `rgba(0,0,0,0.08)` | `rgba(0,0,0,0.30)` | Subtle elevation |
| `--shadow-md` | `rgba(0,0,0,0.12)` | `rgba(0,0,0,0.50)` | Medium elevation |
| `--backdrop` | `rgba(0,0,0,0.40)` | `rgba(0,0,0,0.60)` | Sheet/modal scrim |

**Removed:** `--rust`, `--steel`, `--moss`, `--vignette`, `--grain-blend`, `--grain-opacity`, `--poster-gradient`, `--poster-hi`, `--poster-em`

### Typography

| Token | Value |
|---|---|
| `--font-display` | `"Geist", system-ui, sans-serif` |
| `--font-body` | `"Geist", system-ui, sans-serif` |
| `--font-mono` | `"Geist Mono", ui-monospace, monospace` |

**Note:** Geist is a single variable font covering both display and body weight ranges. `--font-display` and `--font-body` point to the same family; differentiate via `font-weight` (700 display, 400-500 body).

**Removed @font-face:** Fraunces-Variable.woff2, Switzer-Variable.woff2, JetBrainsMono-Regular.woff2, JetBrainsMono-Medium.woff2

**Added @import:** `https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600;700&family=Geist+Mono:wght@400;500&display=swap`

For Tauri/offline: download Geist woff2 files to `packages/ui/src/lib/assets/fonts/` and use `@font-face` instead of `@import`.

### Type Scale

| Token | Value | Change |
|---|---|---|
| `--t-display` | `clamp(40px, 5.4vw, 56px)` | unchanged |
| `--t-h1` | `28px` | unchanged |
| `--t-h2` | `20px` | unchanged |
| `--t-body` | `15px` | unchanged |
| `--t-meta` | `13px` | unchanged |
| `--t-label` | `11px` | unchanged |
| `--t-mono` | `13px` | unchanged |

Letter spacing: add `letter-spacing: -0.02em` on display/h1/h2 usages (in foundry.css heading selectors).

### Radius Scale

| Token | Old | New | Change |
|---|---|---|---|
| `--r-xs` | `4px` | `6px` | Epifly min |
| `--r-sm` | `8px` | `10px` | buttons, pills |
| `--r-md` | `14px` | `14px` | unchanged |
| `--r-lg` | `20px` | `20px` | unchanged |
| `--r-xl` | _(new)_ | `28px` | large cards/modals |
| `--r-full` | `9999px` | `9999px` | unchanged |

### Motion

| Token | Value | Change |
|---|---|---|
| `--dur-1` | `120ms` | unchanged |
| `--dur-2` | `200ms` | unchanged |
| `--dur-3` | `320ms` | unchanged |
| `--dur-4` | `520ms` | unchanged |
| `--ease-out` | `cubic-bezier(0.22, 1, 0.36, 1)` | unchanged |
| `--ease-in` | `cubic-bezier(0.6, 0, 0.7, 0.2)` | unchanged |
| `--ease-spring` | `cubic-bezier(0.34, 1.56, 0.64, 1)` | unchanged |

**Removed aliases:** `--duration-short` → use `--dur-1`, `--duration-base` → use `--dur-2`

### Layout

| Token | Value |
|---|---|
| `--rail` | `240px` (unchanged) |
| `--gutter` | `64px` (unchanged) |
| `--composer-w` | `720px` (unchanged) |

---

## 2. Legacy → Epifly Token Mapping

When executing the refactor, apply these substitutions globally:

| Legacy token | Replace with | Notes |
|---|---|---|
| `var(--ember)` | `var(--ember)` | Keep name, change value to `#FF6200` |
| `var(--ember-2)` | `var(--ember-2)` | Keep name, change value to `#E05500` |
| `var(--ember-soft)` | `var(--ember-soft)` | Keep name, change value |
| `var(--ember-glow)` | `var(--ember-glow)` | Keep name, change value |
| `var(--rust)` | `var(--danger)` | rust was a brownish error color |
| `var(--steel)` | `var(--ink-3)` | steel was a muted blue-grey; use muted ink |
| `var(--ink-muted)` | `var(--ink-3)` | alias that was never defined in tokens |
| `var(--depth)` | Remove; use inline padding-left calc | Only in WorkspaceExplorer; see §4 |
| `var(--stagger-delay)` | Remove; set inline via JS stagger action | Only 1 usage |
| `var(--duration-short)` | `var(--dur-1)` | token alias cleanup |
| `var(--duration-base)` | `var(--dur-2)` | token alias cleanup |
| `var(--vignette)` | Remove `body::before` pseudo-element | Epifly doesn't use editorial vignette |
| `var(--grain-opacity)` / `--grain-blend` | Remove `body::after` pseudo-element | Epifly doesn't use film grain |
| `var(--poster-gradient)` | `linear-gradient(135deg, #FF6200, #E05500 60%, #111111)` | Login page poster, inline or new token |
| `var(--poster-hi)` | `rgba(255,150,80,0.22)` | orange highlight on poster |
| `var(--poster-em)` | `rgba(255,255,255,0.92)` | bright text on poster |
| `var(--backdrop)` | `var(--backdrop)` | Keep, value stays same |
| `var(--shadow-sm)` | `var(--shadow-sm)` | Keep, value updates in dark theme |
| `var(--shadow-md)` | `var(--shadow-md)` | Keep, value stays |
| `var(--r-xs)` | `var(--r-xs)` | Keep, value changes 4→6px |
| `var(--r-sm)` | `var(--r-sm)` | Keep, value changes 8→10px |

---

## 3. File Execution Order

Execute in this order to prevent downstream breakage:

1. **`packages/ui/src/lib/tokens.css`** — rewrite color/font/motion tokens
2. **`packages/ui/src/lib/foundry.css`** — swap @font-face, update token values/references, remove grain/vignette
3. **`packages/ui/src/lib/components/`** — update all component CSS
4. **`packages/ui/src/lib/features/`** — update feature CSS
5. **`packages/ui/src/lib/utils/`** — update utility files
6. **`apps/web/src/`** — update web routes
7. **`apps/browser-shell/src/`** — update mobile shell components

---

## 4. Per-File Checklist

### `packages/ui/src/lib/tokens.css`
**Rewrite entirely.** New content:
- `:root, :root[data-theme="paper"]` — Epifly light palette
- `:root[data-theme="forge"]` — Epifly dark palette  
- `:root` shared — ember orange, cyan, semantic, shadows, fonts, spacing, motion, radius
- Remove: `@media (prefers-color-scheme: dark)` block (we use `data-theme`)
- Keep: `@media (prefers-reduced-motion: reduce)` block

### `packages/ui/src/lib/foundry.css`
- **Fonts:** Replace 4x `@font-face` blocks with Geist @font-face (woff2) or @import
- **Remove:** `--vignette`, `--grain-blend`, `--grain-opacity` token declarations
- **Remove:** `body::before` (vignette pseudo) and `body::after` (grain pseudo)
- **Update:** `--ember` → `#FF6200`, `--ember-2` → `#E05500`, `--ember-soft/glow` → orange rgba
- **Add:** `--cyan`, `--cyan-soft` tokens
- **Update:** `--poster-gradient` → orange gradient
- **Update:** `--r-xs` 4→6px, `--r-sm` 8→10px, add `--r-xl: 28px`
- **Update:** body font-feature-settings (Geist doesn't use same OT features)
- **Remove:** `--rust`, `--steel`, `--moss`
- **Add:** heading letter-spacing `-0.02em` to `.t-display`, `h1`, `h2` selectors if present

### `packages/ui/src/lib/features/auth/LoginPanel.svelte`
- Line 155: `var(--ink-muted)` → `var(--ink-3)`

### `packages/ui/src/lib/features/WorkspaceExplorer.svelte`
- Line 256: `calc(var(--depth) * 16px)` — `--depth` is set inline via JS as `style="--depth:{n}"`. Keep the CSS rule, keep the inline style setter; just ensure `--depth` is not declared as a global token (it isn't — it's component-scoped inline). No change needed.

### `apps/web/src/routes/login/+page.svelte`
- Line 45: `var(--rust)` → `var(--danger)` (error text color)
- `--poster-gradient` usage → update to Epifly orange gradient (or inline)

### All `apps/browser-shell/src/lib/mobile/` files
- Scan for `var(--rust)` → `var(--danger)` (none found in mobile parts, but verify)
- Verify `var(--ember)` usages remain (they will pick up new orange value automatically)

---

## 5. Atmosphere Removal (body pseudo-elements)

The current `foundry.css` applies editorial atmosphere via:
```css
body::before { /* vignette */ background: var(--vignette); }
body::after  { /* grain */   background-image: url("data:image/svg+xml..."); }
```

Epifly's aesthetic is clean/minimal — remove both. If a specific screen needs a gradient hero (e.g. login page), apply it as a scoped element style, not a global body pseudo.

---

## 6. Font Asset Plan

**Option A — CDN (web app):** Add to `apps/web/src/app.html`:
```html
<link rel="preconnect" href="https://fonts.googleapis.com">
<link href="https://fonts.googleapis.com/css2?family=Geist:wght@400;500;600;700&family=Geist+Mono:wght@400;500&display=swap" rel="stylesheet">
```

**Option B — Self-hosted (Tauri/offline):** Download Geist woff2 files, place in `packages/ui/src/lib/assets/fonts/`, use `@font-face`. Required for `apps/browser-shell` (Tauri offline).

Recommended: use Option B for the shared `foundry.css` (works in both contexts). Download files from `https://github.com/vercel/geist-font/releases`.

Fallback stack while fonts load: `system-ui, -apple-system, BlinkMacSystemFont, sans-serif`

---

## 7. Verification Steps

After each phase:

1. `cd packages/ui && npm run check` — zero new type errors
2. `grep -r "var(--rust\|var(--steel\|var(--moss\|var(--ink-muted\|var(--duration-short\|var(--duration-base\|var(--stagger-delay)" apps packages/ui/src` — should return empty
3. Visual check in browser: light mode and dark mode, login page, chat, workspace tree, mobile drawer
4. Confirm ember accent is orange (not teal) on buttons, active states, composer focus ring
5. Confirm Geist font renders on all text (DevTools → Computed → font-family)
6. Confirm no grain/vignette overlay visible on any screen
7. Run `apps/browser-shell` Tauri build — fonts must load offline

---

## 8. Deferred / Out of Scope

- **WorkspaceTree duplication** (`DrawerWorkspaceTree` 466 LOC vs `WorkspaceExplorer` 289 LOC) — headless store extraction is a separate refactor, tracked separately.
- **`--rail`, `--gutter`, `--composer-w`** layout tokens — values unchanged, no migration needed.
- **Semantic color adjustments** (`--success`, `--danger` values) — current values are Epifly-compatible, no change.
- **Icon migration** (emoji → Lucide/Phosphor) — handled by frontend-design skill enforcement, not a token refactor.
