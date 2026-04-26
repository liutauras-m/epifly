# ConusAI Platform — UI Design Guidelines

Editorial, paper-canvas aesthetic. Warm neutral background, single teal accent, clear typographic hierarchy, soft borders, generous whitespace. All tokens live in [src/styles/tokens.css](../src/styles/tokens.css).

---

## 1. Design principles

1. **Editorial, not corporate.** Treat the page like a printed magazine spread — headlines, eyebrows, rules, mono captions.
2. **One accent.** Teal `--accent` is the only saturated colour. Never introduce new hues without updating tokens.
3. **Soft surfaces.** Borders, panels and cards lean on translucency + `backdrop-filter` over hard shadows.
4. **Whitespace earns attention.** Use `clamp()` padding scales; never crowd a section.
5. **Motion is restrained.** Short (220–640ms) easings, always honour `prefers-reduced-motion`.

---

## 2. Colour tokens

| Token | Value | Use |
|---|---|---|
| `--background` | `#f5f1e8` | Page paper canvas |
| `--foreground` | `#1a1814` | Primary text, icons, headings |
| `--theme-color` | `#f5f1e8` | Browser theme-color meta |
| `--accent` | `#80cdc6` | Teal — links, CTAs, highlights |
| `--accent-hover` | `#9dddd7` | Button/link hover |
| `--warning` | `#e53e3e` | Errors, destructive only |
| `--editorial` | `#f97316` | Reserved editorial pop (sparingly) |

### Translucent surface tokens

| Token | Value | Use |
|---|---|---|
| `--border-soft` | `rgba(26,24,20,0.10)` | Card / divider hairlines |
| `--border-strong` | `rgba(26,24,20,0.20)` | Buttons (ghost), key panels |
| `--panel` | `rgba(26,24,20,0.05)` | Subtle inset surfaces |
| `--panel-strong` | `rgba(26,24,20,0.10)` | Hover states on panels |
| `--text-soft` | `rgba(26,24,20,0.80)` | Body copy |
| `--text-muted` | `rgba(26,24,20,0.60)` | Captions, labels, meta |

**Rules**
- Never hard-code hex outside `tokens.css`.
- Tinted accent backgrounds use `rgba(128,205,198,0.10)` (see `.brand-chip--accent`).
- Selection uses `--accent` on `--foreground`.

---

## 3. Typography

Three families, loaded via `next/font` style imports.

| Token | Family | Role |
|---|---|---|
| `--font-display` | `Archivo` | Headings, buttons, eyebrows-strong, nav |
| `--font-sans` | `Inter` | Body copy, UI text |
| `--font-mono` | `Space Mono` | Eyebrows, labels, meta, captions, section numbers |

### Scale & rules

- Base size: `18px` desktop, `16px` ≤640px (`html { font-size }`).
- Headings: `font-weight: 600`, `letter-spacing: -0.02em`, `line-height: 1.1`.
- Body: `font-weight: 400`, `line-height: 1.6`, `letter-spacing: 0.01em`.
- Mono labels: `text-transform: uppercase`, `letter-spacing: 0.10–0.14em`.
- Hero title: `clamp(2.25rem, 4vw, 3.75rem)`.
- Section titles: `clamp(2rem, 4vw, 3rem)`.
- Lead paragraphs: `clamp(0.95rem, 1.1vw, 1.1rem)`, capped at `56ch`.
- Long-form measure: `--measure: 64ch`.

### Utility classes (defined in [components.css](../src/styles/components.css))

- `.brand-heading` — display headline.
- `.brand-body` — body paragraph.
- `.brand-mono` — mono caption.
- `.brand-nav` — uppercase nav label.
- `.brand-eyebrow` — mono eyebrow with muted colour, `0.75rem`, `letter-spacing: 0.12em`.
- `.brand-section-number` — mono accent number prefix (e.g. `01 ⁄`).

---

## 4. Spacing & layout

- Container: `--container: 1200px`, centred via `.container`.
- Section padding: `--section-pad-block: clamp(1.5rem, 3vw, 3rem)`, `--section-pad-inline: clamp(1.25rem, 4vw, 3rem)`.
- Vertical rhythm in sections: `clamp(3rem, 6vw, 5rem)` for hero/footer; standard sections use the section-pad scale.
- Grids: 1-column mobile, break at `560 / 720 / 880 / 980 / 1080px` depending on density.
- Reading width: cap text columns at `52–58ch`.

---

## 5. Border radius scale

Single, consistent set. No arbitrary values.

| Radius | Token / value | Use |
|---|---|---|
| Pill | `999px` | Chips, language switcher segments, circular icon buttons |
| `0.5rem` (8px) | hard-coded | Skip-link, small inset controls, focus outlines |
| `0.75rem` (12px) | hard-coded | Buttons (`.brand-button`), small panels (`.about__stat`) |
| `0.875rem` (14px) | hard-coded | Cards (`.product-card`, `.team-card`) |
| `1rem` (16px) | hard-coded | Large panels (`.brand-panel`, `.hero__side-note`) |
| `50%` | circle | Carousel arrows (`.team-arrow`, 36×36) |

**Rule:** new components pick the closest existing step — do not introduce new radii.

---

## 6. Borders & dividers

- Hairlines: `1px solid var(--border-soft)` — default for cards, rules, footer, between rows.
- Emphasised edges: `1px solid var(--border-strong)` — ghost buttons, hero side-note, key panels.
- Section dividers: `<hr class="brand-rule">` (full-width, `1px`, `--border-soft`).
- Card hover: lift border from `--border-soft` → `--border-strong`.

---

## 7. Surfaces & elevation

No drop shadows. Depth comes from translucency + blur.

| Pattern | Recipe |
|---|---|
| Sticky header | `background: rgba(245,241,232,0.72); backdrop-filter: blur(14px);` border appears on scroll |
| Glass panel (`.brand-panel`) | `rgba(245,241,232,0.72)` + `blur(12px)` + `--border-strong` + `1rem` radius |
| Card surface | `rgba(245,241,232,0.55–0.60)` + `blur(8–12px)` + `--border-soft` |
| Inset panel | `var(--panel)` (no blur) for chips, stats backgrounds |
| Mobile nav overlay | `rgba(245,241,232,0.97)` + `blur(20px)` |
| Footer | `rgba(245,241,232,0.60)` + `blur(10px)` |

Background canvas (`#brand-canvas`) sits at `z-index: 0` with a vignette overlay (`#brand-canvas-overlay` at `z-index: 2`). Content lives at `z-index: 3`.

---

## 8. Components

All component classes are namespaced. Block prefixes: `.brand-*` (primitive), `.site-*` (chrome), `.hero__*` / `.about__*` / `.product-*` / `.team-*` (sections, BEM).

### 8.1 Buttons — `.brand-button`

- Padding: `0.95rem 1.35rem`, radius `0.75rem`, no border.
- Font: display, `600`, `0.8125rem`, uppercase, `letter-spacing: 0.08em`.
- Default: `background: var(--accent)`, text `#1a1814`.
- Hover: `background: var(--accent-hover)`, transition `220ms cubic-bezier(0.4,0,0.2,1)`.
- Variant `--ghost`: transparent background, `1px solid var(--border-strong)`, hover fills with `--panel`.
- Optional arrow span `.brand-button__arrow` translates `+4px` on hover.

### 8.2 Links — `.brand-link`

- Colour `var(--accent)`. Hover: `var(--foreground)` + underline `text-underline-offset: 0.2em`.
- Inline body links inherit `<a>` defaults (foreground → accent on hover).

### 8.3 Chips — `.brand-chip`

- Pill (`999px`), padding `0.2rem 0.55rem`, mono `0.65rem`, uppercase, `letter-spacing: 0.08em`.
- Default surface `--panel` + `--border-soft`.
- Variant `--accent`: accent text + accent border + `rgba(128,205,198,0.10)` fill.

### 8.4 Eyebrow — `.brand-eyebrow`

Mono, `0.75rem`, uppercase, `letter-spacing: 0.12em`, `--text-muted`. Often paired with `.brand-section-number` in accent.

### 8.5 Cards

- **Product card** (`.product-card`): `0.875rem` radius, `--border-soft`, surface `rgba(245,241,232,0.55)`, blur 8px. Header (display name + mono accent tag), body (italic pitch + muted desc, both `-webkit-line-clamp: 2`), footer separated by hairline.
- **Team card** (`.team-card`): `0.875rem` radius, fixed photo aspect `3 / 4`, body padded `0.875rem 1rem 1rem` over hairline divider. Mono accent role label.
- **Stat tile** (`.about__stat`): `0.75rem` radius, `--border-soft`, large display number + mono uppercase label.
- **Side-note** (`.hero__side-note`): `1rem` radius, `--border-strong`, `blur(12px)`, max `36ch`.

### 8.6 Panels & rules

- `.brand-panel` — primary glass panel (see §7).
- `.brand-rule` — `1px` full-width divider in `--border-soft`.

### 8.7 Header — `.site-header`

Sticky, `padding: 1.1rem var(--section-pad-inline)`, glass background. Logo height `28px`. Nav uses `.brand-nav` style tokens. Hamburger collapses below `880px`; mobile overlay slides from top with `380ms cubic-bezier(0.22,1,0.36,1)`.

### 8.8 Language switcher — `.lang-switcher`

Inline-flex pill row, `0.375rem` radius, segmented flags. Inactive flags `opacity: 0.35`, hover `0.7`, active `1` with `8%` foreground tint.

### 8.9 Carousel arrows — `.team-arrow`

`36×36` circle, `1px solid var(--border-strong)`, transparent fill, hover fills with `--panel-strong`. Disabled at `opacity: 0.28`.

### 8.10 Footer — `.site-footer`

Top hairline, glass background, 3-column grid above `720px`. Section titles use mono uppercase muted style.

---

## 9. Iconography & imagery

- Photos: object-fit `cover`, `object-position: center top`. Subtle `scale(1.04)` zoom on card hover (`500ms`).
- Partner logos: `28px` tall, `grayscale(1)` + `opacity: 0.55`, fade to colour on hover.
- Checkmarks in lists rendered as `✓` glyph in mono accent (see `.hero__trust-list`).
- 3D / canvas backgrounds: keep `opacity: 0.9`, fade in via `is-loading` toggle.

---

## 10. Motion

- Standard easing: `cubic-bezier(0.4, 0, 0.2, 1)` for UI (220ms).
- Entrance easing: `cubic-bezier(0.22, 1, 0.36, 1)` for reveals (600–640ms).
- Reveal utilities:
  - `.brand-reveal` — runs `brand-rise` keyframe immediately, with stagger classes `.brand-reveal-1…5` (80ms increments).
  - `.brand-observe` → `.is-in-view` — IntersectionObserver-driven; opacity 0 + 16px translateY.
- Hover micro-interactions: arrow translate `+4px`, photo zoom `1.04`, border darkening.
- All motion gated by `@media (prefers-reduced-motion: reduce)` → `animation: none; transition: none`.

---

## 11. Focus & accessibility

- `:focus-visible` → `2px solid var(--accent)`, `outline-offset: 2px`, `border-radius: 2px`.
- Skip-link `.skip-link` slides in from `top: -100%` to `0` on focus.
- Maintain WCAG AA: foreground (`#1a1814`) on background (`#f5f1e8`) ≈ 14:1. Accent on background fails AA for body text — only use accent for ≥18px display text, icons, or decorative emphasis.
- All interactive controls require visible focus and a hover state.
- Honour `prefers-reduced-motion` everywhere.

---

## 12. Naming & file conventions

- Tokens in [tokens.css](../src/styles/tokens.css), primitives in [components.css](../src/styles/components.css), structural in [layout.css](../src/styles/layout.css), section-specific in [sections.css](../src/styles/sections.css), base resets in [base.css](../src/styles/base.css).
- BEM for sections: `.block__element--modifier`.
- Primitives prefixed `.brand-`.
- State classes: `.is-open`, `.is-active`, `.is-scrolled`, `.is-in-view`, `.is-dragging`, `.is-loading`.

---

## 13. Do / Don't

**Do**
- Reuse existing tokens, radii and component classes.
- Pair display headings with mono eyebrows + numbered prefixes.
- Use translucency + blur for layering.
- Cap text columns at `52–58ch`.

**Don't**
- Add new colours, radii, or shadow stacks.
- Use bold fills outside `--accent`.
- Mix font families inside a single line.
- Animate longer than 640ms or without a reduced-motion fallback.
