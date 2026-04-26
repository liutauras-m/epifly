# ConusAI Platform — UI Design Guidelines

**Foundry** aesthetic: editorial-industrial. Warm cream or carbon-ink ground, one teal accent (`#80cdc6`), hairline rules, generous negative space. All tokens live in [`crates/agent-gateway/assets/css/style.css`](../crates/agent-gateway/assets/css/style.css).

---

## 1. Design Principles

1. **Editorial, not corporate.** Treat the page like a printed workshop spread — headlines, eyebrows, hairline rules, mono captions.
2. **One accent.** Teal `--ember` is the only saturated colour. Never introduce new hues without adding a token.
3. **Intentional corners.** Radius tokens follow the nested-radius principle: inner elements use smaller radii than their outer container (`r_inner ≈ r_outer − gap`). Sharp corners on structural chrome; subtle rounding on interactive surfaces.
4. **Whitespace earns attention.** Use the `--s-*` spacing scale; never crowd a section.
5. **Motion is restrained.** Short (120–520 ms) easings, always honour `prefers-reduced-motion`.

---

## 2. Colour Tokens

### Paper theme (default)

| Token | Value | Use |
|---|---|---|
| `--ink` | `#14110D` | Primary text, near-black, warm undertone |
| `--ink-2` | `#3A332B` | Secondary text |
| `--ink-3` | `#6E6357` | Muted labels, captions |
| `--paper` | `#F4EEE3` | Page background — warm cream |
| `--paper-2` | `#EBE3D4` | Sidebar, raised cards |
| `--paper-3` | `#DFD4BF` | Hover surfaces |
| `--rule` | `#D6CAB0` | Hairline borders, 1 px |
| `--seam` | `#C2B391` | Stronger dividers |

### Forge theme (dark)

| Token | Value |
|---|---|
| `--ink` | `#F4EEE3` |
| `--ink-2` | `#C8BFAE` |
| `--ink-3` | `#8A8174` |
| `--paper` | `#100E0B` |
| `--paper-2` | `#181612` |
| `--paper-3` | `#211E18` |
| `--rule` | `#2A251E` |
| `--seam` | `#3A3328` |

### Shared accents (both themes)

| Token | Value | Use |
|---|---|---|
| `--ember` | `#80cdc6` | Primary accent — teal |
| `--ember-2` | `#5aada6` | Pressed / hover |
| `--ember-soft` | `rgba(128,205,198,0.10)` | Focus rings, chip fills |
| `--ember-glow` | `rgba(128,205,198,0.28)` | Button shadows, cursor glow |
| `--steel` | `#5C6B7A` | Neutral — tool status idle |
| `--rust` | `#8B2E0E` | Error / destructive |
| `--moss` | `#4A6B3A` | Success (legacy; prefer `--success`) |
| `--success` | `#1a7f4b` | Tool success dot |
| `--success-soft` | `rgba(26,127,75,0.13)` | Invoice PAID badge bg |
| `--danger` | `#b32400` | Error |
| `--danger-soft` | `rgba(179,36,0,0.13)` | Invoice OVERDUE badge bg |

**Rules**
- Never hard-code hex outside `style.css`.
- Selection uses `--ember-soft` bg + `--ink` text.

---

## 3. Typography

Three families — loaded via Google Fonts + Fontshare CDN.

| Token | Family | Role |
|---|---|---|
| `--font-display` | `Fraunces` (variable) | Headings, greeting, login tagline |
| `--font-body` | `Switzer` (Fontshare) | Body copy, nav labels, UI text |
| `--font-mono` | `JetBrains Mono` | Eyebrows, labels, tool JSON, code |

### Scale

| Token | Value | Usage |
|---|---|---|
| `--t-display` | `clamp(40px, 5.4vw, 56px)` | Greeting headline |
| `--t-h1` | `28px` | Section titles |
| `--t-h2` | `20px` | Message headers |
| `--t-body` | `15px` | Chat copy |
| `--t-meta` | `13px` | Timestamps, metadata |
| `--t-label` | `11px` | Uppercase mono labels (`letter-spacing: 0.14em`) |
| `--t-mono` | `13px` | Tool JSON, code blocks |

**Greeting** uses `font-variation-settings: "opsz" 96, "SOFT" 30, "WONK" 0` to engage Fraunces' display optical size — distinctive wedge serifs.

---

## 4. Spacing Scale

```css
--s-1: 4px;  --s-2: 8px;  --s-3: 12px; --s-4: 16px;
--s-5: 24px; --s-6: 32px; --s-7: 48px; --s-8: 72px;
--rail: 260px;       /* sidebar width */
--gutter: 64px;      /* main column inset */
--composer-w: 720px; /* max input width */
```

---

## 5. Border Radius Scale

All radii use tokens — no arbitrary values.

| Token | Value | Use |
|---|---|---|
| `--r-xs` | `3px` | Badges, micro elements (inv-badge, plan labels, submit) |
| `--r-sm` | `6px` | Buttons, pills, attachments, toasts, chips, tool cards |
| `--r-md` | `10px` | Composer (outer container), invoice card |
| `--r-lg` | `16px` | Reserved for large panels |
| `--r-full` | `9999px` | Avatar circle, cursor caret, thinking dots |

**Nested radius rule:** send button (`--r-sm`) sits inside the composer (`--r-md`). The gap between them (≈ 4 px) is the difference between the two radii — visually harmonic, following iOS squircle convention.

---

## 6. Motion

```css
--ease-out:    cubic-bezier(0.2, 0.8, 0.2, 1);
--ease-in:     cubic-bezier(0.6, 0, 0.7, 0.2);
--ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1); /* slight overshoot */
--dur-1: 120ms; --dur-2: 200ms; --dur-3: 320ms; --dur-4: 520ms;
```

### Page-load orchestration (staggered)

| Delay | Element |
|---|---|
| `0.08s` | Brand logo |
| `0.16–0.32s` | Sidebar nav sections (cascading) |
| `0.36s` | User chip |
| `0.42s` | Greeting (opacity + 8px rise) |
| `0.56s` | Composer (opacity + 8px rise) |
| `0.68–0.92s` | Quick chips (40 ms stagger) |

### Chat animations

| Moment | Animation |
|---|---|
| User message arrives | `msg-in-user` — slide from right + fade |
| AI message arrives | `msg-in` — rise from below + fade |
| AI streaming | Left rail pulses (traveling ember gradient) |
| Waiting for first token | 3-dot wave (`dot-wave`) |
| Cursor in streaming AI | `cursor-pulse` — scale + opacity + glow |
| Tool card running | Teal border + radial glow ring on dot |
| Tool card done | `card-flash-success` / `card-flash-error` radial pulse |
| View transition | `view-fade-in` — 320ms fade + 4px rise |
| Toast in | `toast-in` — spring scale + fade |
| Send button active | `scale(0.93)` spring rebound |
| Chip hover | `translateY(-1px)` lift + teal border |

**All animations gated by `@media (prefers-reduced-motion: reduce)` → durations clamped to 80 ms.**

---

## 7. Surfaces & Elevation

Depth comes from typography weight and hairline rules — not box shadows.

| Element | Recipe |
|---|---|
| Sidebar | `var(--paper-2)` + `1px solid var(--rule)` right border |
| Sidebar left accent | 1px ember seam (animates `scaleY(0→1)` on load) |
| Composer (rest) | `var(--paper)` + `1px solid var(--rule)` + `border-radius: var(--r-md)` + light `box-shadow` |
| Composer (focus) | `border-color: var(--ember)` + `0 0 0 3px var(--ember-soft)` |
| Composer (chat bottom) | deeper shadow — `0 -2px 24px var(--shadow-sm)` |
| Login poster | `radial-gradient + linear-gradient` teal → dark with noise overlay |

---

## 8. Components

### 8.1 Composer

`border-radius: var(--r-md)` with `overflow: hidden` — child elements clip cleanly. Send button uses `var(--r-sm)` (nested radius). Focus ring follows the outer curve.

### 8.2 Messages

- **User bubble**: `border-radius: 0 var(--r-md) var(--r-md) var(--r-xs)` — sharp top-left (anchors to left), softened elsewhere. Left border `2.5px solid var(--ember)`. Max-width 78%.
- **AI message**: Full-width with `padding-left: var(--s-5)` and a persistent 1.5px left rail (`var(--rule)` at rest, traveling ember gradient while streaming).

### 8.3 Thinking Indicator

Shown immediately when a message is sent, before the first streaming token. Three teal dots in a `dot-wave` stagger (1.3s, delays 0 / 0.18s / 0.36s). Removed automatically when the first token or tool event arrives.

### 8.4 Tool Cards

`border-radius: var(--r-sm)`. Three states:
- **running**: teal border + `0 0 0 2px var(--ember-soft)` glow. Dot is ember with expanding pulse ring (`dot-pulse`).
- **success**: `card-flash-success` radial green pulse → settles. Dot is `var(--success)` with green ring.
- **error**: `card-flash-error` radial rust pulse. Left border 2.5px rust. Dot is `var(--rust)` with danger ring.

### 8.5 Quick Chips

`border-radius: var(--r-sm)`. Transparent border at rest; hover reveals teal border + `var(--ember-soft)` fill + 1px lift. No underline animation (replaced).

### 8.6 Avatar

`border-radius: var(--r-full)` — fully circular, 30×30 px.

### 8.7 Nav Items

`border-radius: var(--r-xs)` on hover/active background. Accent left edge flash: `::before` grows `width: 0 → 2px` with matching `border-radius: 0 var(--r-xs) var(--r-xs) 0`.

### 8.8 Toasts

`border-radius: var(--r-sm)`. Entrance: `toast-in` (`--ease-spring` scale + fade). Exit: opacity + translateY fade-out with `transition`.

### 8.9 Login

Submit button: `border-radius: var(--r-xs)`, lifts `translateY(-1px)` on hover with ember glow. Plan radio labels: `border-radius: var(--r-xs)`, lifts on hover, ember tint when checked.

### 8.10 Invoice Card

`border-top: 3px solid var(--ember)`, `border-radius: 0 0 var(--r-sm) var(--r-sm)` (bottom corners only). Badge: `border-radius: var(--r-xs)`. Shadow: `0 4px 20px var(--shadow-sm)`.

---

## 9. File Structure

```
crates/agent-gateway/
├── assets/
│   ├── css/style.css          # ~1320 lines — design tokens + all components incl. workspace tree
│   ├── js/app.js              # ~660 lines — streaming, animations, composer
│   ├── js/workspace.js        # ~750 lines — tree, search, dialogs, context menu
│   ├── icons/icons.svg        # SVG sprite (one <symbol> per icon)
│   └── images/
│       ├── favicon.png        # Brand sigil (used in head + greeting screen)
│       ├── conusai-logo-lightmode.png
│       └── conusai-logo-darkmode.png
└── templates/
    ├── app.html               # Full shell (sidebar + main + composer + chips)
    ├── login.html             # Split layout — poster + form
    ├── partials/
    │   └── composer.html      # Textarea + toolbar + send button
    └── shared/
        └── head.html          # Meta, fonts, CSS link, theme bootstrap
```

---

## 10. Invoice File Detection

The "Extract invoice" button in the attachment chip only appears when the filename matches **both** conditions:

```js
const INVOICE_EXTS  = /\.(png|jpg|jpeg|pdf)$/i;
const INVOICE_NAMES = /invoice|receipt|bill|facture/i;

function isInvoiceFile(a) {
  return INVOICE_EXTS.test(a.filename) && INVOICE_NAMES.test(a.filename);
}
```

Generic files (e.g. `photo.png`, `report.pdf`) remain plain attachments.

---

## 11. Focus & Accessibility

- `:focus-visible` → `2px solid var(--ember)`, `outline-offset: 2px`.
- `role="log"` on `.messages`; `aria-live="polite" aria-atomic="false"` for streaming.
- `role="status"` + `aria-label="running|complete|error"` on tool card dots.
- All interactive elements: visible focus ring + hover state.
- `prefers-reduced-motion` disables all transforms, clamps durations to 80 ms.
- WCAG AA: `--ink` on `--paper` ≈ 14:1.

---

## 12. Do / Don't

**Do**
- Use `var(--r-*)` tokens — never a raw pixel radius.
- Apply nested radius: inner element radius ≤ outer − gap.
- Reuse `--s-*` spacing scale.
- Gate every animation behind `prefers-reduced-motion`.
- Keep the teal accent purposeful — focus rings, streaming states, interactive signals only.

**Don't**
- Hard-code hex colours outside `style.css`.
- Add new radii, shadow stacks, or colours without adding a token.
- Use `border-radius: 12px` everywhere — this is not a soft/rounded app.
- Animate longer than 520 ms.
- Introduce purple gradients, glass blur panels, or Inter/Roboto.
