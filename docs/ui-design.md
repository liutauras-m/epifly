# ConusAI Platform — UI Design Guidelines

> **Foundry** design system. A **quiet motion system for a premium AI workspace.** Vibrant orange `--ember` + electric cyan `--cyan` accents on a near-neutral ground; Geist + Geist Mono typography; hairline rules; generous negative space; restrained, purposeful motion.
>
> **Taste contract.** Token discipline is not taste. A perfectly tokenized screen can still be cluttered. Every screen, animation, and effect in this system must pass three filters before it ships: (1) **Does it earn the user's attention?** (2) **Would removing it lose meaning?** (3) **Is it the only one of its kind on the screen?** If the answer to any is no, cut it. Minimalism is deciding what *not* to ship — not writing more rules about subtle effects.
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
2. **One accent, one assist — and cyan is sacred.** `--ember` (orange) is the primary saturated colour. `--cyan` appears **only** for live / streaming / system-active states (active token streams, real-time presence dots, in-flight tool indicators). It must **not** be used for generic emphasis, marketing decoration, buttons, hover states, empty-state illustration, charts, info badges, or skeletons. If cyan appears on every surface, the signal dies. Rule of thumb: at most **one** cyan element visible per viewport at any time. No third hue without adding a token.
3. **Intentional corners.** Radii follow the **nested-radius rule** (Apple/iOS squircle convention): an inner element's radius is approximately `r_outer − gap`, where `gap` is the padding between them. Sharp corners on structural chrome; softer on interactive surfaces.
4. **Whitespace earns attention.** Use the `--s-*` scale; never crowd a section.
5. **Motion communicates, never decorates.** Every animation serves one of four purposes — `[feedback]`, `[continuity]`, `[hierarchy]`, `[delight]` ([`ui-plan.md`](ui-plan.md) Principle #14). Anything untaggable doesn't ship. Durations 120–520 ms; `prefers-reduced-motion: reduce` clamps to ≤ 80 ms. See §6.5 (Motion Budget per Screen) — the catalog in §6 is a vocabulary, not a shopping list.
6. **One token, one place.** No hex, no `px` line-heights, no inline shadows outside `tokens.css` / `foundry.css`. CI enforces this from Phase 1.2 onwards (see [`ui-plan.md`](ui-plan.md) §1.2).
7. **Mobile-first.** Author at 360 px; layer up at the `--bp-compact / --bp-medium / --bp-expanded` container-query breakpoints (defined in [`ui-plan.md`](ui-plan.md) Phase 3.1).
8. **A11y is a gate, not a polish.** WCAG 2.2 AA contrast, full keyboard parity, landmark roles, `prefers-reduced-motion`, `prefers-color-scheme`.
9. **Radii are confident, not bubbly.** Structural chrome (page frames, sidebars, table containers) stays sharp or near-sharp. Interactive elements get moderate radii. Only identity / decorative surfaces (avatars, status pills that are genuinely tag-like) reach for `--radius-full`. Premium products feel confident because not everything is rounded into submission.
10. **Glow is a scarce resource.** Soft glows (`--ember-glow`, `--color-accent-soft` rings) are reserved for focus rings, the primary CTA hover, and the streaming cursor. Card-hover glow, persistent live-indicator glow, and ambient "breathing" effects are banned — a premium UI should not look like it's powered by RGB RAM.

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

The canonical semantic layer — components reference **only** these aliases; never the concrete `--ink` / `--paper` / `--ember` tokens directly. Phase 2.1 locked these names; changes must be additive (new token + migration, never rename).

| Token | Paper | Forge | Use |
|---|---|---|---|
| `--color-bg` | `var(--paper)` | `var(--paper)` | Page background |
| `--color-bg-raised` | `var(--paper-2)` | `var(--paper-2)` | Sidebar, raised cards |
| `--color-bg-hover` | `var(--paper-3)` | `var(--paper-3)` | Hover state fill |
| `--color-fg` | `var(--ink)` | `var(--ink)` | Primary text |
| `--color-fg-muted` | `var(--ink-2)` | `var(--ink-2)` | Secondary text |
| `--color-fg-subtle` | `var(--ink-3)` | `var(--ink-3)` | Captions, labels |
| `--color-border` | `var(--rule)` | `var(--rule)` | Hairline borders |
| `--color-accent` | `var(--ember)` | `var(--ember)` | Primary accent — buttons, active states |
| `--color-accent-soft` | `var(--ember-soft)` | `var(--ember-soft)` | Chip fills, soft accent backgrounds |
| `--color-success` | `#1a7f4b` | `#22a060` | Tool success dot, "PAID" badge |
| `--color-success-soft` | `rgba(26, 127, 75, 0.13)` | `rgba(34, 160, 96, 0.15)` | Soft success background |
| `--color-danger` | `#b32400` | `#e03000` | Error states, destructive actions |
| `--color-danger-soft` | `rgba(179, 36, 0, 0.13)` | `rgba(224, 48, 0, 0.15)` | "OVERDUE" badge, error fills |
| `--color-warning` | `#d97706` | `#d97706` | Warning states, quota warnings |
| `--color-warning-text` | `#92400e` | `#92400e` | Warning text on soft backgrounds |
| `--color-warning-soft` | `rgba(217, 119, 6, 0.12)` | `rgba(217, 119, 6, 0.12)` | Warning soft background |
| `--color-warning-border` | `rgba(217, 119, 6, 0.24)` | `rgba(217, 119, 6, 0.24)` | Warning badge border |
| `--color-on-accent` | `#ffffff` | `#ffffff` | Text/icon on accent-filled surfaces |
| `--color-on-danger` | `#ffffff` | `#ffffff` | Text/icon on danger-filled surfaces |
| `--color-on-success` | `#ffffff` | `#ffffff` | Text/icon on success-filled surfaces |

> **Legacy tokens** (`--success`, `--success-soft`, `--danger`, `--danger-soft`) are compatibility aliases pointing at the new `--color-*` equivalents. New code uses `--color-*` exclusively; legacy aliases sunset at Phase 4 close.

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

- **Never hard-code hex outside `tokens.css` / `foundry.css`.** `scripts/check-design-tokens.mjs` fails CI on raw hex anywhere else (active from Phase 1.2).
- **Selection** uses `--ember-soft` background + `--ink` text (set in `foundry.css ::selection`).
- **Use canonical `--color-*` names.** Phase 2.1 migrated all component code from concrete tokens (`--ink`, `--paper`, `--ember`) to semantic aliases (`--color-fg`, `--color-bg`, `--color-accent`). New code uses only `--color-*`. Legacy concrete tokens survive as compatibility aliases in `tokens.css`.
- **On-surface text colors.** When text/icons sit on a solid `--color-accent` / `--color-danger` / `--color-success` fill, use `--color-on-accent` / `--color-on-danger` / `--color-on-success` (always `#ffffff`). Never hard-code `#fff` or `white` in components.

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

Canonical long-form names (use these in new code):

| Token | Value | Usage |
|---|---|---|
| `--font-size-display` | `clamp(40px, 5.4vw, 56px)` | Greeting headline |
| `--font-size-h1` | `28px` | Section titles, page headers |
| `--font-size-h2` | `20px` | Message headers, dialog titles |
| `--font-size-body` | `15px` | Chat copy, paragraphs |
| `--font-size-meta` | `13px` | Timestamps, secondary metadata |
| `--font-size-label` | `11px` | Uppercase mono eyebrows (`letter-spacing: 0.14em`) |
| `--font-size-mono` | `13px` | Tool JSON, inline code |

Short aliases (`--t-display`, `--t-h1` … `--t-mono`) are compatibility shims pointing at the canonical names above. New code uses `--font-size-*`.

### Font family tokens

| Token | Value | Usage |
|---|---|---|
| `--font-family-sans` | `Geist, ui-sans-serif, system-ui` | Body copy, nav labels, UI text |
| `--font-family-display` | `Geist, ui-sans-serif, system-ui` | Display / headlines (same family, different weight axis) |
| `--font-family-mono` | `Geist Mono, ui-monospace, monospace` | Eyebrows, labels, tool JSON, code |

### Component vs. utility split (Phase 2.2)

- `<Type variant="display|h1|h2|label|meta|mono">` from [`packages/ui/src/lib/components/typography/`](../packages/ui/src/lib/components/) is the **only** place `font-variation-settings` lives. Display/h1/h2 variants set the `wght` axis for the rendered size.
- Body copy uses semantic elements (`<p>`, `<li>`) with the token classes `t-body` / `t-body-strong`. **Don't wrap every paragraph in `<Type variant="body">`** — that's component soup with no benefit.
- The discipline is "no inline `font-*` declarations and no untokenized `font-variation-settings`," not "everything is a component."

---

## 4. Spacing Scale

Canonical long-form names (use these in new code):

```css
/* Micro */
--space-px:   1px;    /* single-pixel rule */
--space-half: 2px;    /* tight badge/icon gaps */
/* Core */
--space-1: 4px;   --space-2: 8px;   --space-3: 12px;  --space-4: 16px;
--space-5: 24px;  --space-6: 32px;  --space-7: 48px;  --space-8: 64px;
```

Short aliases (`--s-1` … `--s-8`) are compatibility shims — new code uses `--space-*`.

### Layout tokens (Phase 2.1 / 3.1)

```css
/* Touch targets */
--hit:    44px;   /* standard tap/click target (Apple HIG) */
--hit-sm: 36px;   /* compact icon button */
--hit-xs: 28px;   /* very compact context (toolbar, chips) */

/* Icons */
--icon-xs:  14px;  --icon-sm: 16px;  --icon-md: 20px;  --icon-lg: 24px;

/* Shell */
--sidebar-w:       240px;  /* sidebar full width */
--sidebar-collapsed: 64px; /* icon-only rail */
--topbar-height:    48px;  /* standard topbar */
--topbar-height-compact: 44px;
--topbar-height-expanded: 40px;
--gutter:           64px;  /* main column inset */
--composer-w:      720px;  /* max input width */

/* Component-level */
--chip-h-sm: 24px;   --chip-h-md: 32px;
--dot-sm:     6px;   --dot-md:     8px;
--sheet-handle-w: 40px;  --sheet-handle-h: 4px;
--toast-max-w:   380px;  --toast-dismiss-size: 28px;
--meter-track-h:   6px;
```

---

## 5. Border Radius Scale

Canonical long-form names (use these in new code):

| Token | Value | Use |
|---|---|---|
| `--radius-xs` | `6px` | Badges, micro elements |
| `--radius-sm` | `10px` | Buttons, pills, attachments, toasts, chips, tool cards |
| `--radius-md` | `14px` | Composer outer container, invoice card |
| `--radius-lg` | `20px` | Sheets, large panels, profile cards |
| `--radius-xl` | `28px` | Hero surfaces |
| `--radius-full` | `9999px` | Avatar circle, cursor caret, thinking dots, progress bars |

Short aliases (`--r-xs` … `--r-full`) are compatibility shims. New code uses `--radius-*`.

**Nested radius rule:** the send button (`--radius-sm`, 10 px) sits inside the composer (`--radius-md`, 14 px) with a ~4 px inset gap — `r_outer − gap ≈ r_inner`. Same principle for any nested rounded surface.

---

## 6. Motion

### Easing curves

Canonical names (Phase 2.1 rename, locked):

```css
--ease-standard:   cubic-bezier(0.4, 0, 0.2, 1);     /* neutral state changes */
--ease-out:        cubic-bezier(0.22, 1, 0.36, 1);    /* elements settling into rest (most common) */
--ease-in:         cubic-bezier(0.6, 0, 0.7, 0.2);    /* elements leaving */
--ease-spring:     cubic-bezier(0.34, 1.56, 0.64, 1); /* slight overshoot — confirmations only */

/* Spring physics */
--spring-snappy:   cubic-bezier(0.34, 1.56, 0.64, 1); /* send button press, badge appear */
--spring-gentle:   cubic-bezier(0.28, 0.72, 0.34, 1); /* page reveal settle */
--spring-bouncy:   cubic-bezier(0.22, 1.8, 0.36, 1);  /* delight animations only */
```

### Motion purpose tagging

Every `transition:` or `animation:` must carry a comment tag per [`ui-plan.md`](ui-plan.md) Principle #14:

```css
transition: border-color var(--duration-fast) var(--ease-standard); /* [feedback]    */
transition: transform    var(--duration-slow) var(--ease-out);      /* [continuity]  */
transition: opacity      var(--duration-page) var(--ease-out);      /* [hierarchy]   */
animation:  badge-pop    var(--duration-slow) var(--spring-bouncy); /* [delight]     */
```

`scripts/check-motion-purpose.mjs` enforces tagging — untagged transitions fail CI (error by default, overridable with `MOTION_PURPOSE_ENFORCE=0`).

### Durations

Canonical long-form names (use these in new code):

```css
--duration-fast:    120ms;  /* hover paint, micro-feedback [feedback] */
--duration-normal:  200ms;  /* fast state changes */
--duration-stagger: 240ms;  /* staggered reveals, greeting animations */
--duration-slow:    320ms;  /* sheet open, modal in */
--duration-page:    520ms;  /* page reveals [hierarchy] */
```

Short aliases (`--dur-1` … `--dur-4`) are compatibility shims. New code uses `--duration-*`.

Per-animation hard ceiling: **400 ms**, except the page-load cascade (intentional `[hierarchy]` animation). Per-task total ceiling: **3 000 ms** wall-time, summed across all animations on the user-initiated path. Both enforced in CI from Phase 6 onwards (see [`ui-plan.md`](ui-plan.md) §6).

### Page-load orchestration (cascade)

`[hierarchy]` — guides the eye through the loading shell. Tightened from the original ceremonial cascade: a productivity tool should not introduce every rail section like it's entering a royal wedding.

| Delay | Element |
|---|---|
| `0 ms` | Shell (instant) |
| `80 ms` | Main content frame |
| `120 ms` | Composer (opacity + 8 px rise) |
| `160–280 ms` | Suggestion chips (40 ms stagger, max 4 visible) |

Rail sections, user chip, and brand sigil paint with the shell — no per-element delay. Total cascade ≤ 320 ms; the user can type by 120 ms.

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
| Capability unlock | `[delight]` | **Rare delight.** Spring badge appearance (≤ 320 ms). Non-blocking, non-repeating; fires at most **once per session** and only after a meaningful user milestone — never on routine task completion. |

### Reduced motion

```css
@media (prefers-reduced-motion: reduce) {
  * { animation-duration: 0.01ms !important; transition-duration: 0.01ms !important; }
}
```

Phase 6 replaces the brute `0.01ms` clamp with a deliberate 80 ms opacity cross-fade per animated element, so the UI still acknowledges state changes without motion. Verified by [`apps/web/e2e/visual/reduced-motion.spec.ts`](../apps/web/e2e/visual/) (per [`ui-plan.md`](ui-plan.md) §6).

### 6.5 Motion Budget per Screen

The motion catalog above is a **vocabulary**, not a checklist. Per screen, the budget is:

- **1** primary entrance animation (the cascade, or a route transition — not both).
- **1** persistent state animation maximum at any moment (e.g. streaming rail *or* thinking dots, never simultaneously on the same message).
- Hover / focus feedback **only where interaction affordance is ambiguous**. Buttons, links, and obvious controls do not need to glow on hover.
- **No more than 2** simultaneously animated regions in the user's foveal area (~10° of vision, roughly the active panel).
- Decorative repetition is banned: no constant glow loops, no ambient "breathing" surfaces, no perpetual gradient drifts, no idle bounces.

Motion tiers (use to triage when in doubt):

| Tier | Use | Examples |
|---|---|---|
| Tier 1 — essential | State the user must perceive | Loading, submit, error, success, streaming |
| Tier 2 — spatial | Continuity of place / causality | Drawer, sheet, route transition, FLIP |
| Tier 3 — rare delight | Brand moments | First-load cascade, milestone unlock (once/session) |
| Killed | Decorative repetition | Constant glow loops, repeated bounces, idle pulses |

If a proposed animation doesn't fit Tier 1 or 2, it requires explicit design review before merge.

### Animation library policy

Default is **zero-dep** — Svelte's built-in `transition:` + the keyframes / helpers in [`packages/ui/src/lib/motion/`](../packages/ui/src/lib/motion/) cover ~95% of the surface. **Motion One** (~9 KB) is the only sanctioned escalation, gated on the three triggers in [`ui-plan.md`](ui-plan.md) §2.3. **GSAP and any library > 30 KB are forbidden** — the bundle budget (Phase 8.3) rejects them automatically.

---

## 7. Surfaces & Elevation

Depth comes from typography weight and hairline rules, with minimal shadow assistance.

| Element | Recipe |
|---|---|
| Sidebar / rail | `background: var(--color-bg-raised)` + `border-inline-end: 1px solid var(--color-border)` |
| Sidebar left accent | 1 px `--color-accent` seam (`::before`, animates `scaleY(0→1)` on load) |
| Composer (rest) | `var(--color-bg-raised)` + `border: 1.5px solid var(--color-border)` + `border-radius: var(--radius-lg)` |
| Composer (focus) | `border-color: var(--color-accent)` + `box-shadow: 0 0 0 3px var(--color-accent-soft)` |
| Composer (sticky chat) | Deeper resting shadow: `0 -2px 24px var(--color-shadow-sm)` |
| Card hover lift | `transform: translateY(-1px)` + `box-shadow: 0 4px 12px var(--color-shadow-sm)` |
| Sheet / modal | `box-shadow: 0 8px 24px var(--color-shadow-md)` |
| Login poster | `--poster-gradient` + radial highlight (`--poster-hi`) overlay |

> **RTL / logical properties:** shell borders use `border-inline-end` / `border-inline-start` (not `border-right` / `border-left`) so they mirror correctly in RTL layouts. Layout positions use `inset-inline-start` / `inset-inline-end`. See Phase 7 work in `foundry.css` and `AppShell.svelte`.

---

## 7.5 Screen Composition

Tokens prevent visual chaos; composition rules prevent *organized* visual chaos. Every screen in the product obeys:

- **One dominant visual anchor** per screen — the headline, the composer, the primary card. Everything else is supporting cast.
- **One primary action** per view. Secondary actions are quieter (ghost / link style); destructive actions live in menus or confirmations, not in the primary row.
- **Cards must not compete equally.** Establish hierarchy via size, position, or density — never by adding more color.
- **At most 3 badge types visible in one viewport.** If a screen needs more, the underlying information architecture is wrong.
- **Progressive disclosure beats dense control panels.** Advanced options live behind a `…` / "More" affordance until the user asks for them.
- **Tables vs. cards:** tables for comparison and scanning across many rows; cards for decision-making, summaries, and entities the user will act on individually. Don't render the same data as both on the same screen.
- **Empty states do real work.** They explain what this surface *will* contain, offer one clear next action, and never show a sad illustration without copy. Use `<EmptyState>` from `@conusai/ui`.
- **Loading skeletons mirror final layout** — same number of rows, same column widths. Skeletons that don't match the eventual content read as broken.
- **Text density ceiling.** No body-copy block exceeds ~70 ch line length; no card exceeds ~6 lines of body copy before it should split or summarize.
- **Whitespace before borders.** Reach for `--space-5` / `--space-6` to separate regions before adding a divider. Hairlines are last resort, not default.

Review heuristic: if you can describe what the screen is *for* in one sentence and point to the single element that answers it, the composition is working.

---

## 8. Components

All primitives are exported from [`@conusai/ui`](../packages/ui/src/lib/index.ts). The full gallery is at [`/_/ui`](http://localhost:5173/_/ui) (dev server). Phase 3 ships `Drawer`, `Sheet`, `AppShell`, `AppHeader`, `Sidebar`, `SidebarSection`, `SidebarItem`. Phase 2.7 ships `Button`, `Field`, `Chip`, `EmptyState`. Deprecated aliases (`AppTopBar`, `AppDrawer`, `AppBottomSheet`) are retained with `@deprecated` JSDoc until Phase 4 close.

### 8.1 AppShell (Phase 3.1)

Single shell, named slots: `topbar`, `rail`, `main`, `composer`, `overlay`. Adapts via **container queries** on `app-shell` (not viewport media), so the layout works at any Tauri window size:

| Breakpoint | Threshold | Rail behaviour | Composer placement |
|---|---|---|---|
| Compact | `< --bp-compact` (768 px) | Hidden behind hamburger → drawer slides in from **left** | Fixed bottom |
| Medium | `--bp-medium` (1024 px) | Icons only (`--rail-collapsed: 64px`), expandable on hover | Inline |
| Expanded | `≥ --bp-expanded` (1440 px) | Full `--rail` (240 px), persistent | Inline |

WCAG landmark roles per slot: `<header role="banner">` for `topbar`, `<nav role="navigation">` (or `<aside role="complementary">` per [`docs/ui-landmarks.md`](ui-landmarks.md)) for `rail`, `<main role="main">` for `main`, `<form aria-label="Message composer">` for composer. **Note:** the composer slot is NOT `role="search"` — a chat composer is not a search input; `role="search"` belongs only on `SidebarSearch`. This rule is enforced by `ui:contracts` rule #7.

### 8.2 Composer

`border-radius: var(--radius-md)` outer, `overflow: hidden`. Send button uses `--radius-sm` (nested radius — 14 px outer, 10 px inner, ~4 px gap). States: rest → focus (`var(--color-accent)` border + 3 px `--color-accent-soft` ring) → submitting (skeleton shimmer) → disabled → error. iOS: `font-size: max(16px, var(--font-size-body))` to prevent input zoom. The textarea height grows via the `autoGrow` action ([`packages/ui/src/lib/utils/actions.ts`](../packages/ui/src/lib/utils/actions.ts)).

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

- **Drawer** — slides from the **left** edge (HIG default for primary-nav drawers; never right). Holds rail content on compact viewports. `transform var(--duration-slow) var(--ease-out)` `/* [continuity] */`. RTL: auto-mirrors with `:dir(rtl)` CSS overrides in `Drawer.svelte`.
- **Sheet** — bottom modal on mobile (centered modal on `≥ --bp-compact`), `border-radius: var(--radius-lg) var(--radius-lg) 0 0`. Drag handle: `var(--sheet-handle-w) × var(--sheet-handle-h)`. Both honor `--safe-*` insets.

### 8.10 Toast

`border-radius: var(--radius-sm)`. Max width `var(--toast-max-w)`. Entrance: `toast-in` (spring scale + fade) `/* [feedback] */`. Border-`inline-start` 3 px in the semantic colour (success / danger / warning — uses logical property for RTL). Bottom-stacked on mobile; top-right-stacked on desktop.

> **Note:** `<LiveAnnouncer />` is the SR-only `aria-live` region. **It must not render visible toast UI** — `<ToastHost />` owns that. The current overlap is a known defect tracked for fix in Phase 4.10.

### 8.11 Login

Two-column on `≥ --bp-medium` (1024 px): left = poster (`--poster-gradient` + noise overlay), right = form. Single column on compact; poster shrinks to a 30 vh header.

- Form fields use the `<Field>` primitive (Phase 2.7) with floating label and `aria-describedby` for errors.
- Submit button: `border-radius: var(--radius-md)`, `translateY(-1px)` on hover with `--color-accent-soft` glow.
- Plan radios: `border-radius: var(--radius-md)`, lift on hover, `--color-accent-soft` fill when checked.

### 8.12 Invoice card & InvoiceBadge (Phase 4.5)

Invoice card: `border-top: 3px solid var(--color-accent)`, `border-radius: 0 0 var(--radius-sm) var(--radius-sm)` (bottom only); shadow `0 4px 20px var(--color-shadow-sm)`.

`<InvoiceBadge status="paid|due|overdue">` (wraps `<StatusBadge>` with invoice-domain labeling):

| Status | Background | Text |
|---|---|---|
| `paid` | `--color-success-soft` | `--color-success` |
| `due` | `--color-accent-soft` | `--color-accent` |
| `overdue` | `--color-danger-soft` | `--color-danger` |

### 8.13 StatusBadge (Phase 4.5)

Generic `<StatusBadge status="success|warning|danger|neutral|info" label="…" />` — knows nothing about invoices, capabilities, or billing. Pill shape (`border-radius: var(--radius-full)`), dot indicator `var(--dot-sm)`, uppercase mono label `var(--font-size-label)`.

| Status | Background | Text | Border |
|---|---|---|---|
| `success` | `--color-success-soft` | `--color-success` | `--color-success-soft` |
| `warning` | `--color-warning-soft` | `--color-warning` | `--color-warning-border` |
| `danger` | `--color-danger-soft` | `--color-danger` | `--color-danger-soft` |
| `neutral` | `--color-bg-hover` | `--color-fg-subtle` | `--color-border` |
| `info` | `--cyan-soft` | `--cyan` | `--cyan-soft` |

### 8.14 Sidebar & SidebarSection & SidebarItem (Phase 3.4)

`<Sidebar>` is the adaptive nav rail. Density controlled by the `app-shell` container query:
- **Compact** (`< 768 px`): hidden, accessed via `Drawer`
- **Medium** (`768–1023 px`): icon-only (`--sidebar-collapsed: 64px`)
- **Expanded** (`≥ 1024 px`): full `--sidebar-w: 240px`

`<SidebarSection eyebrow="RECENT">` adds a labeled group with the eyebrow hidden at medium density (too narrow for text).

`<SidebarItem href|onclick active>` renders `<a>` or `<button>`, `border-radius: var(--radius-xs)` on hover, accent left edge `::before` (width 0 → 2 px, `--color-accent`, `/* [feedback] */`). Minimum touch target: `var(--hit)` height.

---

## 9. File Structure

```
packages/ui/
└── src/lib/
    ├── tokens.css                 # Theme + non-theme primitive tokens (source of truth)
    ├── foundry.css                # Self-hosted Geist, reset, shared layout classes
    ├── index.ts                   # Public barrel
    ├── components/                # Primitives (no business logic)
    │   ├── AppShell.svelte        # Single shell w/ named slots (Phase 3.1)
    │   ├── AppHeader.svelte       # Adaptive topbar (Phase 3.3)
    │   ├── Sidebar.svelte         # Nav rail — density via container query (Phase 3.4)
    │   ├── SidebarSection.svelte  # Labeled group within Sidebar
    │   ├── SidebarItem.svelte     # Nav row (<a> or <button>)
    │   ├── Drawer.svelte          # Edge-slide modal via native <dialog> (Phase 3.2)
    │   ├── Sheet.svelte           # Bottom / centered modal via native <dialog> (Phase 3.2)
    │   ├── Button.svelte / Field.svelte / Chip.svelte / EmptyState.svelte  # Phase 2.7
    │   ├── StatusBadge.svelte     # Generic status pill (Phase 4.5)
    │   ├── Type.svelte / Icon.svelte  # Phase 2.2 / 2.4
    │   ├── ThemeProvider.svelte / ThemeSwitcher.svelte
    │   ├── ToastHost.svelte / QuotaBanner.svelte
    │   ├── PlanCard.svelte / PlanBadge.svelte / UsageMeter.svelte
    │   ├── Composer.svelte / CapabilityCard.svelte / WorkspaceTree.svelte
    │   ├── AppTopBar.svelte       # @deprecated → AppHeader (delete Phase 4)
    │   └── AppDrawer.svelte / AppBottomSheet.svelte  # @deprecated (delete Phase 4)
    ├── features/                  # Composed screens & flows (consume primitives + stores)
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

- **Focus ring** — `*:focus-visible` in `foundry.css` applies a global `outline: var(--focus-ring, 2px solid var(--color-accent)); outline-offset: var(--focus-ring-offset, 2px)` to every focusable element. Components override these tokens locally (e.g. `--focus-ring-offset: 4px` on a card) — never hard-code `outline` values. Never use `outline: none` without a token-backed replacement.
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

- Use canonical long-form tokens: `var(--radius-*)`, `var(--space-*)`, `var(--ease-*)`, `var(--duration-*)`, `var(--color-*)`, `var(--font-size-*)` — never a raw value.
- Apply the **nested-radius rule** at every level of nesting.
- Reuse the `--s-*` spacing scale; reach for `--s-7` / `--s-8` before inventing a number.
- Gate every animation behind `prefers-reduced-motion` and tag it with one of the four Principle #14 purposes.
- Keep `--ember` purposeful — focus rings, streaming states, primary actions only. Cyan is reserved for live-data signals.
- Author at 360 px first; let container queries layer up.
- Co-locate a `.fixtures.ts` next to every new primitive (Phase 2.6 — auto-discovered by `/_/ui`).

**Don't**

- Hard-code hex outside `tokens.css` / `foundry.css`.
- Add a new radius, shadow stack, easing curve, or colour without adding a token (then regenerate from `tokens.json` per Phase 2.1).
- Use `border-radius: 12px` everywhere — this is not a soft / rounded app; radii are intentional and discrete. Use `--radius-*` tokens.
- Animate longer than 400 ms per element, or longer than 3 000 ms total per task.
- Add components to `apps/*/src/lib/components/**` — they belong in `packages/ui`. CI enforces from Phase 0.
- Import `lucide-svelte` outside `packages/ui/src/lib/components/icons/`. Use the curated `<Icon>` primitive (Phase 2.4).
- Use viewport `@media` queries on shell layout — use container queries on `app-shell` so Tauri windows of any size work.
- Reintroduce Fraunces / Switzer / JetBrains Mono, purple gradients, glass blur panels, or warm-cream backgrounds. Geist + ember/cyan on near-neutral is the brand.
- Use cyan as a generic accent. Cyan is reserved for live / streaming / active-system signals only (Principle #2).
- Stack multiple persistent animations in the same panel (streaming rail + breathing card + glowing dot). Pick one per moment (§6.5).
- Add hover glow to cards, persistent glow to live dots, or ambient gradient drifts. Glow is reserved for focus, primary CTA hover, and the streaming cursor (Principle #10).
- Show two primary actions on the same screen, or render the same dataset as both a table and a card grid (§7.5).
- Ship a delight animation that fires on routine task completion. Delight is rare, single-session, milestone-only (§6 Chat animations).
- Round structural panels (sidebars, table frames, page chrome) past `--radius-sm`. Confidence comes from sharper structural edges (Principle #9).
