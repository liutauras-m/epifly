---
name: frontend-design
description: Build ConusAI Platform frontend interfaces using the established editorial paper-canvas aesthetic — warm neutrals, single teal accent, Archivo/Inter/Space Mono typography, translucent surfaces. Use this skill when the user asks to build any web component, page, or screen for the ConusAI platform.
license: Complete terms in LICENSE.txt
---

This skill produces production-grade frontend code that is **strictly consistent with the ConusAI design system** defined in [docs/ui-design.md](../../docs/ui-design.md) and the tokens in `src/styles/tokens.css`. Do not invent a new aesthetic per task — every screen must feel like the same magazine.

The user provides frontend requirements: a component, page, application, or interface to build. They may include context about the purpose, audience, or technical constraints. Your job is to render that requirement **inside the existing editorial paper-canvas aesthetic**, not to choose a new one.

---

## 1. Aesthetic anchor (non-negotiable)

ConusAI is **editorial, not corporate**. Treat every page like a printed magazine spread — eyebrows, section numbers, hairline rules, mono captions, generous whitespace, one saturated accent.

| Pillar | Rule |
|---|---|
| Canvas | Warm paper `#f5f1e8` background, dark `#1a1814` foreground (≈14:1 contrast). |
| Accent | Teal `#80cdc6` is the **only** saturated colour. No purple, no gradients on white, no rainbow. |
| Surfaces | Translucency + `backdrop-filter: blur(...)` over hard drop shadows. Never use box-shadow stacks. |
| Type | Three families only: **Archivo** (display), **Inter** (body), **Space Mono** (eyebrows/labels). |
| Motion | 220–640ms, `cubic-bezier(0.4, 0, 0.2, 1)` (UI) or `cubic-bezier(0.22, 1, 0.36, 1)` (reveals). Always honour `prefers-reduced-motion`. |
| Layout | Container `1200px`, asymmetric editorial grids, hairline dividers, `52–58ch` reading measure. |

**Forbidden** — these mark generic AI output and are not allowed in this project:
- New fonts (Space Grotesk, Roboto, system-ui, Arial). Use the three above only.
- New hex values outside `tokens.css`. Always reference `var(--token)`.
- Purple gradients, rainbow gradients, neumorphism, glassmorphism beyond the documented blur recipes.
- Drop shadows. Use translucency + border instead.
- Bold fills in any colour other than `--accent` or `--warning`.
- Centred hero stacks with a giant gradient title (cookie-cutter SaaS layout).

---

## 2. Required workflow before writing code

1. **Read the source of truth.** Open [docs/ui-design.md](../../docs/ui-design.md) and `src/styles/tokens.css` (and `components.css` / `layout.css` / `sections.css` if they exist). Reuse existing tokens and primitives — never reinvent them.
2. **Locate similar existing components.** Search `src/styles/sections.css` and `src/components/` for the closest analogue (card, panel, eyebrow, button). Mirror its structure, naming, and radii.
3. **Pick the radius from the documented scale only**: `999px` pill · `0.5rem` · `0.75rem` · `0.875rem` · `1rem` · `50%`. No new values.
4. **Pick a surface recipe** from §7 of ui-design.md (sticky header, glass panel, card, inset, mobile overlay, footer). Don't invent a new one.
5. **Decide eyebrow + section number prefix.** Most sections lead with `<span class="brand-section-number">01 ⁄</span><span class="brand-eyebrow">…</span>`.
6. **Then write code.**

---

## 3. Tokens you MUST use

```css
/* Colour */
var(--background)      /* #f5f1e8 — page canvas */
var(--foreground)      /* #1a1814 — text, icons, headings */
var(--accent)          /* #80cdc6 — links, CTAs */
var(--accent-hover)    /* #9dddd7 */
var(--warning)         /* #e53e3e — destructive only */
var(--editorial)       /* #f97316 — editorial pop, sparingly */
var(--border-soft)     /* rgba(26,24,20,0.10) */
var(--border-strong)   /* rgba(26,24,20,0.20) */
var(--panel)           /* rgba(26,24,20,0.05) */
var(--panel-strong)    /* rgba(26,24,20,0.10) */
var(--text-soft)       /* body copy */
var(--text-muted)      /* captions, labels */

/* Typography */
var(--font-display)    /* Archivo */
var(--font-sans)       /* Inter */
var(--font-mono)       /* Space Mono */

/* Layout */
var(--container)             /* 1200px */
var(--section-pad-block)     /* clamp(1.5rem, 3vw, 3rem) */
var(--section-pad-inline)    /* clamp(1.25rem, 4vw, 3rem) */
var(--measure)               /* 64ch */
```

If a value you need is not in `tokens.css`, add it there first — never inline a hex/px constant in a component.

---

## 4. Required typographic patterns

- **Hero / page title** — `.brand-heading`, Archivo 600, `letter-spacing: -0.02em`, `clamp(2.25rem, 4vw, 3.75rem)`.
- **Section title** — `.brand-heading`, `clamp(2rem, 4vw, 3rem)`, paired with eyebrow + section number above.
- **Eyebrow row** — Space Mono uppercase, `0.75rem`, `letter-spacing: 0.12em`, `--text-muted`. Pair with `.brand-section-number` in `--accent` (`01 ⁄`, `02 ⁄`, …).
- **Lead paragraph** — Inter 400, `clamp(0.95rem, 1.1vw, 1.1rem)`, `line-height: 1.6`, capped at `56ch`.
- **Body paragraph** — `.brand-body`, capped at `--measure` (`64ch`).
- **Mono caption / meta / label** — `.brand-mono`, uppercase, `letter-spacing: 0.10–0.14em`.
- **Inline links** — inherit foreground, `--accent` on hover with `text-underline-offset: 0.2em`.

Never mix two font families on the same line.

---

## 5. Component recipes (use these instead of inventing)

| Need | Use | Notes |
|---|---|---|
| Primary CTA | `.brand-button` | Teal fill, Archivo uppercase 0.8125rem, radius `0.75rem`. Append `<span class="brand-button__arrow">→</span>` for forward actions. |
| Secondary CTA | `.brand-button .brand-button--ghost` | Transparent + `--border-strong`. |
| Inline tag / status | `.brand-chip` (or `--accent` variant) | Pill, mono uppercase 0.65rem. |
| Eyebrow | `.brand-eyebrow` | Often with `.brand-section-number`. |
| Glass panel | `.brand-panel` | `1rem` radius, blur 12px, `--border-strong`. |
| Divider | `<hr class="brand-rule">` | Full-width hairline. |
| Card (content) | `.product-card` pattern | `0.875rem` radius, blur 8–12px, `--border-soft`, line-clamped body. |
| Card (person) | `.team-card` pattern | `3 / 4` photo, mono accent role label. |
| Stat tile | `.about__stat` pattern | Display number + mono uppercase label. |
| Side note | `.hero__side-note` pattern | `1rem` radius, `--border-strong`, blur 12px, `max-width: 36ch`. |

For any new section, follow the BEM block convention `.section-name__element--modifier` (see `.hero__*`, `.about__*`, `.product-*`, `.team-*`).

---

## 6. Motion patterns

- **Reveal on load** — `.brand-reveal` plus stagger `.brand-reveal-1…5` (80ms increments).
- **Reveal on scroll** — `.brand-observe` toggled to `.is-in-view` by an IntersectionObserver. Opacity 0 → 1, translateY 16px → 0, 600–640ms `cubic-bezier(0.22, 1, 0.36, 1)`.
- **Hover** — arrow translates `+4px`, photo `scale(1.04)` over 500ms, border darkens `--border-soft` → `--border-strong`.
- **Sticky header** — toggles `.is-scrolled` to add a hairline.
- **Reduced motion** — every animation/transition must be inside (or accompanied by) `@media (prefers-reduced-motion: reduce) { animation: none; transition: none; }`.

Never animate longer than 640ms. Never animate hue.

---

## 7. Accessibility (must pass)

- `:focus-visible` → `2px solid var(--accent)`, `outline-offset: 2px`, `border-radius: 2px`.
- Skip link `.skip-link` slides from `top: -100%` to `0` on focus.
- Touch targets ≥ `44×44px`.
- Don't put body text in `--accent` — fails AA on the paper canvas. Accent is for ≥18px display text, icons, or decorative emphasis only.
- Always render visible labels — never icon-only controls without an `aria-label` AND a tooltip.
- Don't convey state via colour alone (pair with icon, glyph, or text).

---

## 8. File / naming conventions

- Tokens → `src/styles/tokens.css`
- Primitives → `src/styles/components.css` (`.brand-*`)
- Structure → `src/styles/layout.css`
- Section-specific → `src/styles/sections.css` (BEM `.block__element--modifier`)
- Base resets → `src/styles/base.css`
- State classes only: `.is-open`, `.is-active`, `.is-scrolled`, `.is-in-view`, `.is-dragging`, `.is-loading`.

When adding a new section, append its CSS to `sections.css` under a clearly labelled `/* === <Section name> === */` block. When adding a new primitive, append it to `components.css` with a `.brand-*` prefix.

---

## 9. Self-review checklist (run before finishing)

- [ ] All colours reference `var(--…)` tokens — zero inline hex.
- [ ] Only Archivo / Inter / Space Mono are used.
- [ ] Radius matches the documented scale.
- [ ] No `box-shadow` (translucency + border instead).
- [ ] No purple gradients, no rainbow, no Inter-as-display.
- [ ] Section starts with eyebrow + section number + display heading.
- [ ] Reading measure capped at `52–58ch`.
- [ ] `prefers-reduced-motion` fallback present.
- [ ] `:focus-visible` styled.
- [ ] BEM/`brand-*` naming respected.
- [ ] Hover and focus states defined for every interactive element.

If any box is unchecked, fix it before returning code.

---

## 10. When the request conflicts with the system

If the user asks for something that breaks these rules (e.g. "make the hero a purple gradient", "use Space Grotesk"), **flag the conflict and propose the on-brand equivalent**. Do not silently break the design system. Examples:

- Asked for a "vibrant gradient hero" → offer a teal `--accent` accent panel with editorial typography and a translucent background sketch, not a saturated gradient.
- Asked to "use Inter for headings" → push back; headings are Archivo. Inter is body.
- Asked for "drop shadows on cards" → use the documented translucency + `--border-soft` recipe.

Stay polite, state the rule, and offer the on-brand alternative.

---

## Reference

- Full spec: [docs/ui-design.md](../../docs/ui-design.md)
- Tokens: `src/styles/tokens.css`
- Primitives: `src/styles/components.css`
- Sections: `src/styles/sections.css`