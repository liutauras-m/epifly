# ConusAI Platform — UI Design Guidelines

> **Foundry** design system. Vibrant orange `--ember` + electric cyan `--cyan` accents on a near-neutral ground; Geist + Geist Mono typography; hairline rules; generous negative space; restrained, purposeful motion.
>
> **Single source of truth:** [`packages/ui/src/lib/tokens.css`](../packages/ui/src/lib/tokens.css) (theme + non-theme primitive tokens) and [`packages/ui/src/lib/foundry.css`](../packages/ui/src/lib/foundry.css) (resets, self-hosted fonts, shared layout classes). Every screen — web (`apps/web`), Tauri desktop, and mobile (`apps/browser-shell` on iOS/Android) — consumes the **same** tokens and the **same** primitives from `@conusai/ui`. Zero per-app design forks.
>
> **Companion docs:**
> - [`docs/ui-plan.md`](ui-plan.md) — the migration plan that turns these guidelines into ship reality (phases, gates, audit cadence).
> - [`docs/ui-landmarks.md`](ui-landmarks.md) — WCAG 2.2 landmark map (created in `ui-plan.md` Phase 3.1).
>
> When this doc and the code disagree, **the code wins** and this doc is the bug. Open a PR to reconcile.

---

## 1. Design Principles

1. **Editorial, not corporate.** Treat each screen like a printed workshop spread — generous whitespace, mono eyebrows, hairline rules, headlines that earn their size.
2. **One accent, one assist.** `--ember` (orange) is the primary saturated colour; `--cyan` is the secondary signal for streaming / live-data. No third hue without adding a token.
3. **Intentional corners.** Radii follow the **nested-radius rule** (Apple/iOS squircle convention): an inner element's radius is approximately `r_outer − gap`, where `gap` is the padding between them. Sharp corners on structural chrome; softer on interactive surfaces.
4. **Whitespace earns attention.** Use the `--s-*` scale; never crowd a section.
5. **Motion communicates, never decorates.** Every animation serves one of four purposes — `[feedback]`, `[continuity]`, `[hierarchy]`, `[delight]` ([`ui-plan.md`](ui-plan.md) Principle #14). Anything untaggable doesn't ship. Durations 120–520 ms; `prefers-reduced-motion: reduce` clamps to ≤ 80 ms.
6. **One token, one place.** No hex, no `px` line-heights, no inline shadows outside `tokens.css` / `foundry.css`. CI enforces this from Phase 1.2 onwards (see [`ui-plan.md`](ui-plan.md) §1.2).
7. **Mobile-first.** Author at 360 px; layer up at the `--bp-compact / --bp-medium / --bp-expanded` container-query breakpoints (defined in [`ui-plan.md`](ui-plan.md) Phase 3.1).
8. **A11y is a gate, not a polish.** WCAG 2.2 AA contrast, full keyboard parity, landmark roles, `prefers-reduced-motion`, `prefers-color-scheme`.

---

## 2. Colour Tokens

Theme tokens live under `:root[data-theme="paper"]` (light, default) and `:root[data-theme="forge"]` (dark). Non-theme tokens (brand, accents, semantics, overlays) live under `:root`, with overrides in the `forge` block where the dark surface needs more lift.

### Paper theme (light, default)

| Token | Value | Use |
|---|---|---|
| `--ink` | `#111111` | Primary text |
| `--ink-2` | `#3A3A3A` | Secondary text |
| `--ink-3` | `#767676` | Muted labels, captions |
| `--paper` | `#F8F8F8` | Page background |
| `--paper-2` | `#F0F0F0` | Sidebar, raised cards |
| `--paper-3` | `#E8E8E8` | Hover surfaces |
| `--rule` | `#E0E0E0` | Hairline borders, 1 px |
| `--seam` | `#C8C8C8` | Stronger dividers |

### Forge theme (dark)

| Token | Value |
|---|---|
| `--ink` | `#F8F8F8` |
| `--ink-2` | `#C0C0C0` |
| `--ink-3` | `#888888` |
| `--paper` | `#111111` |
| `--paper-2` | `#1A1A1A` |
| `--paper-3` | `#222222` |
| `--rule` | `#2A2A2A` |
| `--seam` | `#3A3A3A` |

### Brand accent — ember (orange)

| Token | Paper value | Forge value | Use |
|---|---|---|---|
| `--ember` | `#FF6200` | `#FF6200` | Primary accent — saturated orange |
| `--ember-2` | `#E05500` | `#FF7A20` | Pressed / hover; brighter on dark |
| `--ember-soft` | `rgba(255, 98, 0, 0.10)` | `rgba(255, 98, 0, 0.12)` | Focus rings, chip fills |
| `--ember-glow` | `rgba(255, 98, 0, 0.22)` | `rgba(255, 98, 0, 0.28)` | Button shadows, cursor glow |

### Secondary accent — cyan (electric)

| Token | Paper value | Forge value | Use |
|---|---|---|---|
| `--cyan` | `#00D4FF` | `#00D4FF` | Streaming indicators, "live" badges |
| `--cyan-soft` | `rgba(0, 212, 255, 0.10)` | `rgba(0, 212, 255, 0.12)` | Live-data backgrounds |

### Semantic tokens

| Token | Paper | Forge | Use |
|---|---|---|---|
| `--success` | `#1a7f4b` | `#22a060` | Tool success dot, "PAID" badge |
| `--success-soft` | `rgba(26, 127, 75, 0.13)` | `rgba(34, 160, 96, 0.15)` | Soft success background |
| `--danger` | `#b32400` | `#e03000` | Error states, destructive actions |
| `--danger-soft` | `rgba(179, 36, 0, 0.13)` | `rgba(224, 48, 0, 0.15)` | "OVERDUE" badge, error fills |

### Overlays & elevation

| Token | Paper | Forge | Use |
|---|---|---|---|
| `--shadow-sm` | `rgba(0, 0, 0, 0.08)` | `rgba(0, 0, 0, 0.30)` | Resting card lift |
| `--shadow-md` | `rgba(0, 0, 0, 0.12)` | `rgba(0, 0, 0, 0.50)` | Sheet / modal lift |
| `--backdrop` | `rgba(0, 0, 0, 0.40)` | `rgba(0, 0, 0, 0.60)` | Drawer / dialog backdrop |

### Login poster (paper + forge share)

| Token | Value | Use |
|---|---|---|
| `--poster-gradient` | `linear-gradient(135deg, #FF6200 0%, #E05500 60%, #111111 100%)` | Login left pane |
| `--poster-hi` | `rgba(255, 150, 80, 0.22)` | Radial highlight overlay |
| `--poster-em` | `rgba(255, 255, 255, 0.92)` | Tagline copy on poster |

### Rules

- **Never hard-code hex outside `tokens.css` / `foundry.css`.** Phase 1.2's `scripts/check-design-tokens.mjs` fails CI on raw hex anywhere else.
- **Selection** uses `--ember-soft` background + `--ink` text (set in `foundry.css ::selection`).
- **Semantic aliases** (`--bg`, `--bg-raised`, `--fg`, `--fg-muted`, `--border`, `--accent`, `--danger`, `--success`) land in [`ui-plan.md`](ui-plan.md) Phase 2.1. After they ship, components reference **only** the aliases; theme files remap them. Until then, components use the concrete tokens above.

---

## 3. Typography

Two self-hosted families (CSP-safe, Tauri-offline-friendly) loaded via `@font-face` in [`foundry.css`](../packages/ui/src/lib/foundry.css) from [`packages/ui/src/lib/assets/fonts/`](../packages/ui/src/lib/assets/fonts/).

| Token | Family | Variable axes | Role |
|---|---|---|---|
| `--font-display` | `Geist` (variable) | `wght` 100–900 | Display, headlines, greetings |
| `--font-body` | `Geist` (variable) | `wght` 100–900 | Body copy, nav labels, UI text |
| `--font-mono` | `Geist Mono` (variable) | `wght` 100–900 | Eyebrows, labels, tool JSON, code |

> **Typography lock:** Per [`ui-plan.md`](ui-plan.md) Phase 0.4, the choice of Geist is final. The sentinel comment at the top of `foundry.css` enforces it. Earlier brainstorming around Fraunces / Switzer / JetBrains Mono is retired — do not reintroduce.

### Scale

| Token | Value | Usage |
|---|---|---|
| `--t-display` | `clamp(40px, 5.4vw, 56px)` | Greeting headline |
| `--t-h1` | `28px` | Section titles, page headers |
| `--t-h2` | `20px` | Message headers, dialog titles |
| `--t-body` | `15px` | Chat copy, paragraphs |
| `--t-meta` | `13px` | Timestamps, secondary metadata |
| `--t-label` | `11px` | Uppercase mono eyebrows (`letter-spacing: 0.14em`) |
| `--t-mono` | `13px` | Tool JSON, inline code |

### Component vs. utility split (Phase 2.2)

- `<Type variant="display|h1|h2|label|meta|mono">` from [`packages/ui/src/lib/components/typography/`](../packages/ui/src/lib/components/) is the **only** place `font-variation-settings` lives. Display/h1/h2 variants set the `wght` axis for the rendered size.
- Body copy uses semantic elements (`<p>`, `<li>`) with the token classes `t-body` / `t-body-strong`. **Don't wrap every paragraph in `<Type variant="body">`** — that's component soup with no benefit.
- The discipline is "no inline `font-*` declarations and no untokenized `font-variation-settings`," not "everything is a component."

---

## 4. Spacing Scale

```css
--s-1: 4px;   --s-2: 8px;   --s-3: 12px;  --s-4: 16px;
--s-5: 24px;  --s-6: 32px;  --s-7: 48px;  --s-8: 64px;

--rail:        240px;   /* sidebar full width */
--gutter:       64px;   /* main column inset */
--composer-w:  720px;   /* max input width */
```

Future layout tokens (`--rail-collapsed: 64px`, `--hit: 44px`, `--safe-top/-bottom/-left/-right`, `--bp-compact / -medium / -expanded`) land in [`ui-plan.md`](ui-plan.md) Phase 2.1 / 3.1.

---

## 5. Border Radius Scale

| Token | Value | Use |
|---|---|---|
| `--r-xs` | `6px` | Badges, micro elements |
| `--r-sm` | `10px` | Buttons, pills, attachments, toasts, chips, tool cards |
| `--r-md` | `14px` | Composer outer container, invoice card |
| `--r-lg` | `20px` | Sheets, large panels, profile cards |
| `--r-xl` | `28px` | Hero surfaces |
| `--r-full` | `9999px` | Avatar circle, cursor caret, thinking dots, progress bars |

**Nested radius rule:** the send button (`--r-sm`, 10 px) sits inside the composer (`--r-md`, 14 px) with a ~4 px inset gap — `r_outer − gap ≈ r_inner`. Same principle for any nested rounded surface.

---

## 6. Motion

### Easing curves

```css
--ease-out:    cubic-bezier(0.22, 1, 0.36, 1);     /* default — elements settling into rest */
--ease-in:     cubic-bezier(0.6, 0, 0.7, 0.2);     /* elements leaving */
--ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1);  /* slight overshoot — confirmations only */
--ease-ui:     cubic-bezier(0.4, 0, 0.2, 1);       /* legacy Material ease — neutral state changes */
```

Phase 2.1 of [`ui-plan.md`](ui-plan.md) renames these to `--ease-standard / --ease-emphasized-decelerate / --ease-emphasized-accelerate / --ease-linear` and introduces spring physics tokens (`--spring-snappy`, `--spring-gentle`, `--spring-bouncy`). Treat the rename as a strict superset — same physics, clearer vocabulary.

### Durations

```css
--dur-1:  120ms;  /* hover paint, micro-feedback */
--dur-2:  200ms;  /* fast state changes */
--dur-2b: 240ms;  /* staggered reveals, greeting animations */
--dur-3:  320ms;  /* sheet open, modal in */
--dur-4:  520ms;  /* page reveals */
```

Per-animation hard ceiling: **400 ms**, except the page-load cascade (intentional `[hierarchy]` animation). Per-task total ceiling: **3 000 ms** wall-time, summed across all animations on the user-initiated path. Both enforced in CI from Phase 6 onwards (see [`ui-plan.md`](ui-plan.md) §6).

### Page-load orchestration (cascade)

`[hierarchy]` — guides the eye through the loading shell so each region announces itself in turn.

| Delay | Element |
|---|---|
| `80 ms` | Brand logo / sigil |
| `160–320 ms` | Rail sections (cascading) |
| `360 ms` | User chip |
| `420 ms` | Greeting (opacity + 8 px rise) |
| `560 ms` | Composer (opacity + 8 px rise) |
| `680–920 ms` | Suggestion chips (40 ms stagger) |

Total cascade ≤ 920 ms, well under the 3 s task budget.

### Chat animations

Each entry is tagged with its purpose per [`ui-plan.md`](ui-plan.md) Principle #14.

| Moment | Tag | Animation |
|---|---|---|
| User message arrives | `[feedback]` | `msg-in-user` — slide from right + fade |
| AI message arrives | `[continuity]` | `msg-in` — rise from below + fade |
| AI streaming | `[continuity]` | Left rail traveling ember gradient (~1.4 s loop) |
| Waiting for first token | `[continuity]` | 3-dot wave (`dot-wave`, 1.3 s, 0 / 0.18 / 0.36 s stagger) |
| Cursor in streaming AI | `[continuity]` | `cursor-pulse` — scale + opacity + glow (1 s loop) |
| Tool card running | `[continuity]` | Ember border + `0 0 0 2px var(--ember-soft)` glow ring on dot |
| Tool card done | `[feedback]` | `card-flash-success` / `card-flash-error` radial pulse (~280 ms) |
| View transition | `[continuity]` | `document.startViewTransition` → fallback `view-fade-in` (200 ms) |
| Toast in | `[feedback]` | `toast-in` — spring scale + fade |
| Send button press | `[feedback]` | `scale(0.93)` spring rebound (~180 ms, `--spring-snappy`) |
| Chip hover (pointer-fine) | `[feedback]` | `translateY(-1px)` + ember border (120 ms) |
| Capability unlock | `[delight]` | Spring badge appearance (≤ 320 ms, once per session per milestone) |

### Reduced motion

```css
@media (prefers-reduced-motion: reduce) {
  * { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; }
}
```

Phase 6 replaces the brute `0.01ms` clamp with a deliberate 80 ms opacity cross-fade per animated element, so the UI still acknowledges state changes without motion. Verified by [`apps/web/e2e/visual/reduced-motion.spec.ts`](../apps/web/e2e/visual/) (per [`ui-plan.md`](ui-plan.md) §6).

### Animation library policy

Default is **zero-dep** — Svelte's built-in `transition:` + the keyframes / helpers in [`packages/ui/src/lib/motion/`](../packages/ui/src/lib/motion/) cover ~95% of the surface. **Motion One** (~9 KB) is the only sanctioned escalation, gated on the three triggers in [`ui-plan.md`](ui-plan.md) §2.3. **GSAP and any library > 30 KB are forbidden** — the bundle budget (Phase 8.3) rejects them automatically.

---

## 7. Surfaces & Elevation

Depth comes from typography weight and hairline rules, with minimal shadow assistance.

| Element | Recipe |
|---|---|
| Sidebar / rail | `background: var(--paper-2)` + `border-right: 1px solid var(--rule)` |
| Sidebar left accent | 1 px `--ember` seam (`::before`, animates `scaleY(0→1)` on load) |
| Composer (rest) | `var(--paper-2)` + `border: 1.5px solid var(--rule)` + `border-radius: var(--r-lg)` |
| Composer (focus) | `border-color: var(--ember)` + `box-shadow: 0 0 0 3px var(--ember-soft)` |
| Composer (sticky chat) | Deeper resting shadow: `0 -2px 24px var(--shadow-sm)` |
| Card hover lift | `transform: translateY(-1px)` + `box-shadow: 0 4px 12px var(--shadow-sm)` |
| Sheet / modal | `box-shadow: 0 8px 24px var(--shadow-md)` |
| Login poster | `--poster-gradient` + radial highlight (`--poster-hi`) overlay |

---

## 8. Components

The current production primitives are exported from [`@conusai/ui`](../packages/ui/src/lib/index.ts). [`ui-plan.md`](ui-plan.md) Phase 3 promotes the chrome (`AppTopBar`, `AppDrawer`, `AppBottomSheet`) into top-level primitives (`TopBar` / `Drawer` / `Sheet`), and Phase 2.7 extracts the missing cross-cutting primitives (`Button`, `Field`, `Chip`, `EmptyState`). Naming aliases per Principle #13: `Rail ↔ Sidebar`, `TopBar ↔ Header`.

### 8.1 AppShell (Phase 3.1)

Single shell, named slots: `topbar`, `rail`, `main`, `composer`, `overlay`. Adapts via **container queries** on `app-shell` (not viewport media), so the layout works at any Tauri window size:

| Breakpoint | Threshold | Rail behaviour | Composer placement |
|---|---|---|---|
| Compact | `< --bp-compact` (768 px) | Hidden behind hamburger → drawer slides in from **left** | Fixed bottom |
| Medium | `--bp-medium` (1024 px) | Icons only (`--rail-collapsed: 64px`), expandable on hover | Inline |
| Expanded | `≥ --bp-expanded` (1440 px) | Full `--rail` (240 px), persistent | Inline |

WCAG landmark roles per slot: `<header role="banner">` for `topbar`, `<nav role="navigation">` (or `<aside role="complementary">` per [`docs/ui-landmarks.md`](ui-landmarks.md)) for `rail`, `<main role="main">` for `main`, `<form role="search">` for composer.

### 8.2 Composer

`border-radius: var(--r-md)` outer, `overflow: hidden`. Send button uses `--r-sm` (nested radius — 14 px outer, 10 px inner, ~4 px gap). States: rest → focus (`var(--ember)` border + 3 px `--ember-soft` ring) → submitting (skeleton shimmer) → disabled → error. iOS: `font-size: max(16px, var(--t-body))` to prevent input zoom. The textarea height grows via the `autoGrow` action ([`packages/ui/src/lib/utils/actions.ts`](../packages/ui/src/lib/utils/actions.ts)).

### 8.3 Messages

- **User bubble** — `border-radius: 0 var(--r-md) var(--r-md) var(--r-xs)` (sharp top-left anchors to left edge); 2.5 px `var(--ember)` left rail; `max-width: 78%`.
- **AI message** — full-width with `padding-left: var(--s-5)`; persistent 1.5 px left rail (`var(--rule)` at rest, traveling-ember gradient while streaming).

`<MessageList role="log" aria-live="polite">` keeps the latest message in scroll lock unless the user scrolls away (then a "jump to latest" pill appears).

### 8.4 Thinking Indicator

Three `--ember` dots in a `dot-wave` stagger. Shown immediately on submit; removed automatically on the first streaming token *or* the first tool event.

### 8.5 Tool Cards

`border-radius: var(--r-sm)`. Three states (the dot in the head signals state):

| State | Border | Dot | Animation |
|---|---|---|---|
| running | `var(--ember)` + 2 px `--ember-soft` glow | `var(--ember)` + expanding pulse ring | `dot-pulse` loop |
| success | `var(--rule)` | `var(--success)` + soft ring | `card-flash-success` radial (~280 ms) |
| error | `var(--danger)` left 2.5 px | `var(--danger)` + danger ring | `card-flash-error` radial |

`role="status"` on the dot; `aria-label` carries the state.

### 8.6 Chip / Suggestion chip

`border-radius: var(--r-full)`, transparent border at rest. Hover (pointer-fine only): `--ember-glow` border + `--paper-3` fill + `translateY(-1px)`. Entry animation: `chip-in` (220 ms, `--ease-out`, opacity + 8 px rise). Stagger 40 ms when rendered in a list.

### 8.7 Avatar

`border-radius: var(--r-full)`. Default 28 × 28 px in the rail, 48 × 48 px on the account page. Ember tint background (`--ember-soft`), ember-glow border, `--ember-2` text.

### 8.8 Nav Items (rail)

`border-radius: var(--r-xs)` on hover / active background. Accent left edge: `::before` grows from `width: 0 → 2px` on hover, color `var(--ember)`, with matching `border-radius: 0 var(--r-xs) var(--r-xs) 0`.

### 8.9 Drawer & Sheet

Both are backed by the native `<dialog>` element with a focus trap and `inert` background (Phase 3.2). Both carry explicit `aria-modal="true"` + `aria-label` (or `aria-labelledby`) — required props on the Svelte component, so the type checker fails the build if a consumer omits them.

- **Drawer** — slides from the **left** edge (HIG default for primary-nav drawers; never right). Holds rail content on compact viewports. `transform var(--dur-3) var(--ease-out)`.
- **Sheet** — bottom modal on mobile (centered modal on `≥ --bp-compact`), `border-radius: var(--r-lg) var(--r-lg) 0 0`. Drag handle on mobile. Both honor `--safe-*` insets.

### 8.10 Toast

`border-radius: var(--r-sm)`. Entrance: `toast-in` (spring scale + fade). Border-left 3 px in the semantic colour (success / danger / warning). Bottom-stacked on mobile (above the home indicator via `env(safe-area-inset-bottom)`); top-right-stacked on desktop (above `--bp-compact`).

> **Note:** `<LiveAnnouncer />` is the SR-only `aria-live` region. **It must not render visible toast UI** — `<ToastHost />` owns that. The current overlap ([`packages/ui/src/lib/utils/LiveAnnouncer.svelte`](../packages/ui/src/lib/utils/LiveAnnouncer.svelte)) is a known defect tracked for fix in Phase 4.10.

### 8.11 Login

Two-column on `≥ --bp-medium` (1024 px): left = poster (`--poster-gradient` + noise overlay), right = form. Single column on compact; poster shrinks to a 30 vh header.

- Form fields use the `<Field>` primitive (Phase 2.7) with floating label and `aria-describedby` for errors.
- Submit button: `border-radius: var(--r-md)`, `translateY(-1px)` on hover with ember glow.
- Plan radios: `border-radius: var(--r-md)`, lift on hover, `--ember-soft` fill when checked.

### 8.12 Invoice card & InvoiceBadge (Phase 4.5)

Invoice card: `border-top: 3px solid var(--ember)`, `border-radius: 0 0 var(--r-sm) var(--r-sm)` (bottom only); shadow `0 4px 20px var(--shadow-sm)`.

`<InvoiceBadge status="paid|due|overdue">`:

| Status | Background | Text |
|---|---|---|
| `paid` | `--success-soft` | `--success` |
| `due` | `--ember-soft` | `--ember` |
| `overdue` | `--danger-soft` | `--danger` |

---

## 9. File Structure

```
packages/ui/
└── src/lib/
    ├── tokens.css                 # Theme + non-theme primitive tokens (source of truth)
    ├── foundry.css                # Self-hosted Geist, reset, shared layout classes
    ├── index.ts                   # Public barrel
    ├── components/                # Primitives (no business logic)
    │   ├── AppShell.svelte        # Single shell w/ named slots — built in Phase 3.1
    │   ├── ThemeProvider.svelte
    │   ├── ThemeSwitcher.svelte
    │   ├── ToastHost.svelte
    │   ├── PlanCard.svelte / PlanBadge.svelte / UsageMeter.svelte / QuotaBanner.svelte
    │   ├── WorkspaceTree.svelte
    │   └── (Phase 2.7) Button, Field, Chip, EmptyState
    ├── features/                  # Composed screens & flows (consume primitives + stores)
    │   ├── chrome/                # AppTopBar, AppDrawer, AppBottomSheet (moves to components/ in Phase 0.1)
    │   ├── AgentChatComposer.svelte / AgentChatStream.svelte / ToolCallCard.svelte
    │   ├── CapabilityBrowser.svelte / CapabilityRow.svelte / CapabilityPinChip.svelte
    │   ├── WorkspaceExplorer.svelte / DrawerRecentChats.svelte
    │   ├── SuggestionChips.svelte / ContextChip.svelte / HostedProjectCard.svelte
    │   └── screens/               # ChatScreen, CapabilitiesScreen, ArtifactsScreen, etc.
    ├── motion/                    # springAnimate, recordRect/playFlip, stagger, tap, viewTransition
    ├── stores/                    # Theme, drawer, screen, recents, breadcrumbs, toast, mode, featureFlags
    ├── utils/                     # actions (autoGrow), motion-prefs, md, LiveAnnouncer (SR-only)
    ├── routing/                   # initialRoute, applyInitialRoute (deep-link handling)
    └── assets/                    # fonts/ (Geist Variable woff2), icons/, images/ (sigil, logos)

apps/
├── web/                           # SvelteKit web build (Chromium target)
│   └── src/routes/                # +layout.svelte mounts <ThemeProvider>+<LiveAnnouncer>+<ToastHost>
└── browser-shell/                 # Tauri 2 shell (iOS, Android, macOS, Windows)
    ├── src/lib/mobile/            # MobileShell.svelte + parts/ (first consumer of AppShell in Phase 3.1)
    └── src-tauri/                 # Rust side, capabilities/, tauri.conf.json
```

> **Components vs features (Principle #10 of [`ui-plan.md`](ui-plan.md)):** `components/` = primitives with no business logic; `features/` = composed screens that pull from stores or the SDK. A file graduates from `features/` → `components/` only when its API has no app-domain coupling left. New code defaults to `components/` unless it composes ≥ 2 primitives **and** reads app state.

---

## 10. Invoice File Detection (composer attachments)

The "Extract invoice" affordance on an attachment chip appears only when both conditions match — keeps the UI clean for generic uploads.

```ts
const INVOICE_EXTS  = /\.(png|jpg|jpeg|pdf)$/i;
const INVOICE_NAMES = /invoice|receipt|bill|facture/i;

function isInvoiceFile(a: Attachment) {
  return INVOICE_EXTS.test(a.filename) && INVOICE_NAMES.test(a.filename);
}
```

`photo.png` and `report.pdf` remain plain attachments; `invoice-2026-05.pdf` exposes the extraction action.

---

## 11. Focus & Accessibility

- **Focus ring** — `outline: 2px solid var(--ember); outline-offset: 2px;` on every `:focus-visible`. The unified `--focus-ring: 0 0 0 3px var(--ember-soft)` shadow ring lands in Phase 7. Never remove an outline without a token-backed replacement.
- **Skip links** — "Skip to main", "Skip to composer" injected by `AppShell` (Phase 7).
- **ARIA** — `<MessageList role="log" aria-live="polite">`, `<ToolCard role="status">` on the state dot, dialogs use native `<dialog>` + explicit `aria-modal="true"` + `aria-label` (Principle #13).
- **Landmarks** — `banner`, `navigation` (or `complementary`), `main`, `contentinfo`, `search` — exactly one each per screen unless documented in [`docs/ui-landmarks.md`](ui-landmarks.md). axe-core 0 violations is a CI gate.
- **Keyboard parity** — every mouse action reproducible by keyboard. `/` focuses composer; `Cmd/Ctrl+K` opens command palette; `Esc` closes any sheet / drawer; `Cmd/Ctrl+N` opens a new chat.
- **Contrast** — WCAG 2.2 AA on both themes. `--ink` on `--paper` measures ≈ 16:1 (paper) / ≈ 17:1 (forge).
- **Reduced motion** — every animation clamps to the 80 ms cross-fade described in §6.
- **Touch targets** — every interactive element ≥ 44 × 44 pt (Apple HIG); ≥ 48 × 48 dp on Android. Enforced by Playwright assertions per Phase 4 audit.
- **i18n** — all user-visible strings wrapped in `t('key')` from `packages/ui/utils/i18n.ts`. RTL mirrors the layout (rail to the right).

---

## 12. Do / Don't

**Do**

- Use `var(--r-*)`, `var(--s-*)`, `var(--ease-*)`, `var(--dur-*)` tokens — never a raw value.
- Apply the **nested-radius rule** at every level of nesting.
- Reuse the `--s-*` spacing scale; reach for `--s-7` / `--s-8` before inventing a number.
- Gate every animation behind `prefers-reduced-motion` and tag it with one of the four Principle #14 purposes.
- Keep `--ember` purposeful — focus rings, streaming states, primary actions only. Cyan is reserved for live-data signals.
- Author at 360 px first; let container queries layer up.
- Co-locate a `.fixtures.ts` next to every new primitive (Phase 2.6 — auto-discovered by `/_/ui`).

**Don't**

- Hard-code hex outside `tokens.css` / `foundry.css`.
- Add a new radius, shadow stack, easing curve, or colour without adding a token (then regenerate from `tokens.json` per Phase 2.1).
- Use `border-radius: 12px` everywhere — this is not a soft / rounded app; radii are intentional and discrete.
- Animate longer than 400 ms per element, or longer than 3 000 ms total per task.
- Add components to `apps/*/src/lib/components/**` — they belong in `packages/ui`. CI enforces from Phase 0.
- Import `lucide-svelte` outside `packages/ui/src/lib/components/icons/`. Use the curated `<Icon>` primitive (Phase 2.4).
- Use viewport `@media` queries on shell layout — use container queries on `app-shell` so Tauri windows of any size work.
- Reintroduce Fraunces / Switzer / JetBrains Mono, purple gradients, glass blur panels, or warm-cream backgrounds. Geist + ember/cyan on near-neutral is the brand.
