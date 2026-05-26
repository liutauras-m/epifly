---
name: frontend-design
description: Build Epifly frontend interfaces using the established premium 2026 AI/SaaS aesthetic — vibrant orange + electric cyan accents on warm off-white (light) or deep charcoal (dark), Geist Sans typography, soft 14–28px radii, translucent surfaces with subtle shadows and cyan glows. Always places shared code in `packages/ui` so web (apps/web), desktop and mobile (apps/browser-shell — Tauri 2 + iOS + Android) consume identical components, tokens, motion primitives, and stores. Use this skill when the user asks to build any web component, page, or screen for the Epifly platform.
license: Complete terms in LICENSE.txt
---

This skill produces production-grade frontend code that is **strictly consistent with the Epifly brand system** defined in [docs/branding/branding.md](../../docs/branding/branding.md) and the reference layouts in [docs/branding/index.html](../../docs/branding/index.html) and [docs/branding/mobile.html](../../docs/branding/mobile.html). Do not invent a new aesthetic per task — every screen must feel like the same product.

The user provides frontend requirements: a component, page, application, or interface to build. Your job is to render that requirement **inside the existing premium-AI aesthetic**, not to choose a new one.

---

## 1. Brand anchor (non-negotiable)

**Epifly** — tagline **"Instant insight. Effortless flow."** Concept: *epiphany on the fly* — sudden clarity and effortless speed. The aesthetic sits in the **Linear · Vercel · Claude · Perplexity** tier of premium 2026 AI products: ultra-clean, high-contrast, buttery-smooth, emotionally uplifting, never flashy.

| Pillar | Rule |
|---|---|
| Light canvas | Warm off-white `#F8F8F8` background with soft radial orange + cyan glows in corners. |
| Dark canvas | Deep charcoal `#111111` background with brighter radial cyan + orange glows. |
| Primary accent | Vibrant Orange `#FF6200` — wordmark, logo, primary CTA, eyebrows. |
| Secondary accent | Electric Cyan `#00D4FF` — typing dots, hover glows, focus halos, secondary highlights. |
| Foreground | `#111111` on light, `#F8F8F8` on dark. ≈19:1 contrast either way. |
| Type | Two families only: **Geist** (display + body + UI) and **Geist Mono** (code, eyebrows, mono captions). |
| Surfaces | Translucent panels (`backdrop-filter: blur(18px)`) over the canvas, with soft shadow elevation. In dark mode shadows become neon glows (orange or cyan). |
| Radii | `14px` small · `18px` medium · `20px` large · `28px` hero · `999px` pill. No other values. |
| Motion | Spring physics. UI: 150–300ms `cubic-bezier(0.4, 0, 0.2, 1)`. Reveals/messages: 300ms `cubic-bezier(0.34, 1.56, 0.64, 1)`. Always honour `prefers-reduced-motion`. |
| Layout | Container `min(1240px, calc(100vw - 32px))`, generous whitespace, asymmetric grids on the hero, perfectly kerned headings. |

**Forbidden** — these mark off-brand or generic 2024 SaaS output and are not allowed:
- **Emoji as UI iconography** (`✨ 🚀 ✅ 📊 ⚡ 🎯`). Emoji rendering is OS-dependent, breaks contrast against the orange/cyan palette, and reads as a 2018 indie-product tell. Use Lucide / Phosphor strokes instead — see §6 Iconography.
- New fonts (Inter, Roboto, Space Grotesk, system-ui, Arial). Geist + Geist Mono only.
- New hex values outside the palette. Always reference `var(--token)`.
- Purple/violet/teal-as-primary. Orange and cyan are the only saturated colours.
- Rainbow gradients. Multi-stop linear gradients across the full hue wheel.
- Hard drop shadows (e.g. `0 4px 6px rgba(0,0,0,0.3)`). Use the documented soft elevation or cyan glow recipes.
- Fully rounded blobs (`border-radius: 50px` on a card). Stay inside the 14–28px scale.
- Centred hero stacks with a giant gradient title — this is the cookie-cutter 2023 SaaS layout we reject.
- Body text inside a saturated fill (orange button text okay; orange paragraph not okay).
- Cosmetic motion (rotating gradients, looping pulses on idle elements, parallax-for-its-own-sake). Every animation must carry meaning — see §9 Motion.

---

## 2. Required workflow before writing code

1. **Read the source of truth.** Open [docs/branding/branding.md](../../docs/branding/branding.md) and [docs/branding/index.html](../../docs/branding/index.html). Skim the `:root` token block and the section patterns (hero, palette, type, principles).
2. **Locate similar existing components.** Search `packages/ui/src/lib/` and `apps/*/src/` for the closest analogue (card, panel, eyebrow, button, message bubble). Mirror its structure, naming, and radii. If the project's `tokens.css` does not yet contain the Epifly tokens, add them there first — never inline.
3. **Decide *where* this code lives — see §11 Cross-platform sharing.** Default placement is `packages/ui`. Only fall back to an app folder when the component depends on a platform-only API (Tauri `invoke`, SvelteKit `$app/*`, WKWebView bridges). This step happens **before** picking a filename.
4. **Pick the radius from the documented scale only**: `14px` · `18px` · `20px` · `28px` · `999px`. No new values.
5. **Pick a surface recipe** from §7 (sticky topbar, hero copy panel, hero stage panel, mini-card grid, prompt card, variant card, mobile message bubble). Don't invent a new one.
6. **Decide light-or-dark or both.** Every component must look correct in *both* modes. Use the `body[data-theme="dark"]` selector pattern from `index.html` or `@media (prefers-color-scheme: dark)`.
7. **Lead the section with an eyebrow** — a Geist Mono uppercase `0.74rem` `letter-spacing: 0.14em` line, often in orange (`var(--orange)`), optionally preceded by a `44px` linear-gradient hairline `linear-gradient(90deg, var(--orange), transparent)`.
8. **Then write code.**

---

## 3. Tokens you MUST use

```css
:root {
  /* Colour — palette */
  --orange:        #FF6200;   /* primary accent, CTAs, logo, eyebrows */
  --orange-soft:   #FF8B47;   /* hover, secondary orange surfaces */
  --cyan:          #00D4FF;   /* glow, focus, typing dots, secondary highlights */
  --charcoal:      #111111;   /* dark canvas, dark mode foreground inverse */
  --off-white:     #F8F8F8;   /* light canvas */
  --indigo:        #1E3A8A;   /* optional deep accent — sparingly */
  --paper:         #FFF6EF;   /* warmest off-white, hero stage */

  /* Colour — neutrals */
  --line:          rgba(17,17,17,0.12);
  --line-strong:   rgba(17,17,17,0.20);
  --text-soft:     rgba(17,17,17,0.72);
  --text-muted:    rgba(17,17,17,0.56);
  --surface:       rgba(255,255,255,0.72);
  --surface-strong:rgba(255,255,255,0.88);

  /* Elevation */
  --shadow:        0 24px 60px rgba(17,17,17,0.08);
  --shadow-cta:    0 18px 38px rgba(255,98,0,0.24);

  /* Radii */
  --radius-sm:     14px;
  --radius-md:     18px;
  --radius-lg:     20px;
  --radius-hero:   28px;
  --radius-pill:   999px;

  /* Layout */
  --container:     min(1240px, calc(100vw - 32px));

  /* Typography */
  --font-sans:     "Geist", system-ui, sans-serif;
  --font-mono:     "Geist Mono", ui-monospace, monospace;

  /* Motion */
  --ease-ui:       cubic-bezier(0.4, 0, 0.2, 1);
  --ease-spring:   cubic-bezier(0.34, 1.56, 0.64, 1);
  --ease-slide:    cubic-bezier(0.32, 0.72, 0, 1);
}

body[data-theme="dark"] {
  --line:          rgba(248,248,248,0.12);
  --line-strong:   rgba(248,248,248,0.20);
  --text-soft:     rgba(248,248,248,0.76);
  --text-muted:    rgba(248,248,248,0.58);
  --surface:       rgba(22,22,22,0.72);
  --surface-strong:rgba(19,19,19,0.86);
  --shadow:        0 26px 68px rgba(0,0,0,0.42);
}
```

If a value you need is not in the token set, add it to `packages/ui/src/lib/tokens.css` first — never inline a hex or px constant in a component.

---

## 4. Canvas (page background)

The body is never a flat colour. Always layer two radial glows over a near-flat linear gradient — this is the signature "Epifly canvas".

**Light:**
```css
background:
  radial-gradient(circle at top left, rgba(0,212,255,0.10), transparent 34%),
  radial-gradient(circle at 85% 12%, rgba(255,98,0,0.16), transparent 28%),
  linear-gradient(180deg, #FFFDFA 0%, #FFF4EB 40%, #FFF 100%);
```

**Dark:**
```css
background:
  radial-gradient(circle at 15% 10%, rgba(0,212,255,0.18), transparent 25%),
  radial-gradient(circle at 78% 14%, rgba(255,98,0,0.20), transparent 32%),
  linear-gradient(180deg, #090909 0%, #111111 44%, #171717 100%);
```

Never use a single solid hex for `body`.

---

## 5. Required typographic patterns

- **Hero / page title** — `var(--font-sans)`, `font-weight: 800`, `letter-spacing: -0.06em`, `line-height: 0.94`, `font-size: clamp(4rem, 8vw, 7.8rem)`. Cap at `10ch` width to force the editorial column break.
- **Section title** — `var(--font-sans)`, `font-weight: 800`, `letter-spacing: -0.06em`, `font-size: clamp(2.2rem, 4vw, 4rem)`. Always preceded by an `.eyebrow` (orange Geist Mono, `0.74rem`, `letter-spacing: 0.14em`).
- **Wordmark "Epifly"** — Geist Sans `font-weight: 700`, `letter-spacing: -0.04em`, paired with the orange swallow/starburst logo at `34px`.
- **Lead paragraph** — Geist Sans 400, `font-size: clamp(1.05rem, 1.3vw, 1.2rem)`, `line-height: 1.7`, `color: var(--text-soft)`, capped at `34rem` / `56ch`.
- **Body paragraph** — Geist Sans 400, `line-height: 1.65`, `color: var(--text-soft)`, capped at `44rem` / `64ch`.
- **Eyebrow / overline / mono meta** — Geist Mono `0.74rem` `text-transform: uppercase` `letter-spacing: 0.14em`. In orange when leading a section; in `--text-muted` for table/metric labels.
- **Section tag with hairline** — eyebrow preceded by a `44×1px` `linear-gradient(90deg, var(--orange), transparent)`. See `.section-tag::before` in `index.html`.
- **UI label / button** — Geist Sans `font-weight: 600`, normal letter-spacing.
- **Code / mono inline** — `var(--font-mono)`, weight 500.

Heading letter-spacing rule: `-0.02em` for UI labels, `-0.04em` for the wordmark, `-0.06em` for display headings. Never use positive tracking on headings.

Never mix two font families on the same line. Never set Geist Mono on body paragraphs.

---

## 6. Iconography — premium icons only, never emoji

Epifly's iconographic language is **single-line stroke icons, 1.5–1.75px stroke, 24px nominal grid, round line caps + joins, `currentColor` fill** — the convention shared by Linear, Vercel, Notion, Claude, and Perplexity. Use a vetted icon set; never paste emoji into product UI.

### 6.1 Approved icon libraries (in order of preference)

| Library | When to use | Install |
|---|---|---|
| **Lucide** | Default. Largest set, perfectly tuned for premium AI UIs, MIT licensed. Used by shadcn/ui, Vercel, Linear-ish products. | `npm i lucide-svelte` / `lucide-react` |
| **Phosphor Icons** | When you need multiple visual weights (regular / bold / duotone / fill) inside the same surface, or icon-as-illustration in onboarding. | `npm i phosphor-svelte` / `@phosphor-icons/react` |
| **Tabler Icons** | Power-user / developer-tools regions (charts, code, terminals). | `npm i @tabler/icons-svelte` |
| **Custom Epifly SVGs** | The orange swallow/starburst mark and product-defining glyphs only. Author with the same stroke discipline as Lucide. | Hand-author in `docs/branding/` or `packages/ui/src/lib/icons/`. |

Pick **one** library per surface — never mix Lucide and Phosphor stroke icons in the same component, the proportions clash.

### 6.2 Stroke + size discipline

```svelte
<!-- ✅ Right — currentColor inherits foreground, sizes from font -->
<Sparkles size={20} strokeWidth={1.75} aria-hidden="true" />

<!-- ❌ Wrong — hard-coded colour breaks theme switching -->
<Sparkles size={20} stroke="#FF6200" />
```

- **Stroke width:** `1.5px` for `≥24px` icons, `1.75px` for `16–20px` icons. Never use `2px` (Material default — too heavy for this brand) or `1px` (too thin against translucent surfaces).
- **Size scale:** `14`, `16`, `18`, `20`, `24`, `28`, `32px`. Match icon size to the line-height of adjacent text (icon equals cap-height plus 2px on either side).
- **Colour:** always `currentColor`. The parent element controls hue via `color: var(--…)`.
- **Alignment:** `display: inline-flex; align-items: center; gap: 0.5em;` on the parent. Never offset icons with `margin-top: 2px` hacks — fix the gap and line-height instead.
- **Accessibility:** decorative icons get `aria-hidden="true"`; standalone icon buttons get `aria-label="…"` plus a tooltip on hover/focus.

### 6.3 Semantic colour mapping

| Meaning | Colour token | Example glyphs |
|---|---|---|
| Brand / primary action | `var(--orange)` | `Sparkles`, `Rocket`, `Send`, `ArrowUpRight` (used inside primary CTAs at white-on-orange) |
| Information / hover / focus | `var(--cyan)` | `Info`, `Search`, `Command`, `Sparkles` (used as halo glyph) |
| Neutral UI | `currentColor` (`--charcoal` / `--off-white`) | `Settings`, `User`, `ChevronDown`, `MoreHorizontal` |
| Success | `#16A34A` (Tailwind green-600) | `Check`, `CheckCircle2` |
| Warning | `#D97706` (amber-600 — *not* the brand orange) | `AlertTriangle`, `Clock` |
| Destructive | `#DC2626` (red-600) | `Trash2`, `X`, `AlertOctagon` |

Status colours live outside the brand palette intentionally — using `--orange` for "warning" would conflict with its primary-action role.

### 6.4 Canonical icon → meaning mapping

Use these glyphs consistently across the product. If a meaning is needed that's not listed, pick the closest Lucide name and document the choice in `packages/ui/src/lib/icons/README.md`.

| Action | Lucide glyph | Notes |
|---|---|---|
| Send message | `Send` or `ArrowUp` | `ArrowUp` for compact composers, `Send` for full chat. |
| New chat / compose | `PenLine` or `SquarePen` |  |
| Search | `Search` | Pair with `⌘K` mono hint on the right edge. |
| Settings | `Settings` (gear) |  |
| User / account | `CircleUser` |  |
| Sign in / sign out | `LogIn` / `LogOut` |  |
| Theme toggle | `Sun` ↔ `Moon` | Morph via 200ms crossfade, *not* rotate. |
| Expand / collapse | `ChevronDown` / `ChevronUp` | Rotate 180° over 180ms `var(--ease-ui)`. |
| Next / forward | `ArrowRight` or `ArrowUpRight` | `ArrowUpRight` inside CTAs ("learn more ↗"). |
| Close | `X` | 16px in dialog headers, 20px in side panels. |
| Confirm / done | `Check` |  |
| Copy | `Copy` → swap to `Check` for 1.4s on success. | Standard copy-to-clipboard pattern. |
| Delete / destructive | `Trash2` |  |
| Info tooltip | `Info` |  |
| AI / generate | `Sparkles` | The only acceptable "magic" glyph. Never `✨`. |
| Attach file | `Paperclip` |  |
| Image | `Image` (Lucide) |  |
| Voice input | `Mic` / `MicOff` |  |
| Loading | Lucide `Loader2` with `animate-spin` — never a spinning emoji. | Or the typing-dots recipe in §7. |

### 6.5 Implementation patterns

**Svelte / SvelteKit:**

```svelte
<script>
  import { Sparkles, ArrowUpRight } from 'lucide-svelte';
</script>

<button class="cta" type="button">
  <Sparkles size={18} strokeWidth={1.75} aria-hidden="true" />
  <span>Generate insight</span>
  <ArrowUpRight size={16} strokeWidth={1.75} aria-hidden="true" />
</button>
```

**React (when applicable):**

```tsx
import { Sparkles, ArrowUpRight } from "lucide-react";

<button className="cta">
  <Sparkles size={18} strokeWidth={1.75} aria-hidden />
  <span>Generate insight</span>
  <ArrowUpRight size={16} strokeWidth={1.75} aria-hidden />
</button>
```

**Bare HTML / CSS:** import the SVG inline (never `<img src="…icon.svg">` — it can't inherit `currentColor`).

### 6.6 Emoji policy

- **Forbidden in:** product UI labels, buttons, navigation, tooltips, empty states, error messages, status badges, marketing pages, OG cards, document titles.
- **Acceptable in:** chat *message content* the user or agent wrote, code comments, CHANGELOG entries, commit messages, and external comms. The user's freely-typed messages are theirs to render verbatim — but our chrome around them is icon-only.

If a designer-spec includes an emoji (`✨ Generate`), replace it with `<Sparkles />` and the same label, then note the substitution.

---

## 7. Surface & component recipes (use these instead of inventing)

| Need | Recipe | Notes |
|---|---|---|
| **Sticky topbar** | `position: sticky; top: 16px;` · `border-radius: 999px;` · `background: var(--surface);` · `border: 1px solid var(--line);` · `backdrop-filter: blur(18px);` · `box-shadow: var(--shadow);` · `padding: 16px 18px;` · `grid-template-columns: auto 1fr auto;` | Brand lockup left, nav centre, theme toggle right. |
| **Hero copy panel** | `border-radius: var(--radius-hero);` · `background: var(--surface);` · `border: 1px solid var(--line);` · `backdrop-filter: blur(18px);` · `padding: 40px;` · `min-height: 620px;` · flex column with `justify-content: space-between` | Houses the eyebrow, h1, lead, CTA row, metrics row. |
| **Hero stage panel** | Same as hero copy panel but split into a light `stage-panel` (gradient `rgba(255,255,255,0.92) → rgba(255,244,235,0.9)`) and a `dark-panel` (gradient `rgba(17,17,17,0.96) → rgba(33,33,33,0.9)`) side-by-side at `1.15fr 0.85fr`. | Demonstrates light + dark identity at once. |
| **Section card** | `border-radius: var(--radius-hero);` · `background: var(--surface);` · `border: 1px solid var(--line);` · `backdrop-filter: blur(18px);` · `padding: 28px;` · `box-shadow: var(--shadow);` | Containers for palette, principles, type, prompts, variants. |
| **Mini-card / variant-card** | `border-radius: 22px;` · `background: rgba(255,255,255,0.56)` (light) / `rgba(255,255,255,0.04)` (dark) · `border: 1px solid var(--line);` · `padding: 16px;` | Grid item — palette swatch, principle, type sample. |
| **Primary CTA** | `background: var(--orange);` · `color: #FFF;` · `border-radius: 18px;` · `padding: 14px 18px;` · `font-weight: 600;` · `box-shadow: var(--shadow-cta);` · hover: `transform: translateY(-2px);` | Includes an arrow glyph (`→` or `↗`) with 10px gap on the right. |
| **Ghost / secondary CTA** | `background: transparent;` · `border: 1px solid var(--line-strong);` · `color: inherit;` · same radius, padding, weight, hover lift. | Use beside the primary CTA. |
| **Pill chip** | `border-radius: 999px;` · `padding: 6px 12px;` · Geist Mono `0.74rem` uppercase `letter-spacing: 0.14em` · border `--line`. | For status badges and small inline tags. |
| **Metric tile** | `padding: 14px 16px;` · `border-radius: var(--radius-md);` · `background: rgba(255,255,255,0.42)` (light) / `rgba(255,255,255,0.03)` (dark) · `border: 1px solid var(--line);` · `<strong>` for the number (Geist Sans `1.2rem`, `letter-spacing: -0.04em`), `<span>` for the mono uppercase label. | Three-up grid below the hero. |
| **Message bubble (mobile)** | `border-radius: 18px;` · animation `messageIn` 300ms `var(--ease-spring)` (scale 0.95 + translateY 12px → 1 + 0, opacity 0 → 1). | Send-side bubble uses orange fill `#FF6200` + white text; receive-side uses `var(--surface)`. |
| **Typing indicator** | Three `6×6px` cyan dots, `border-radius: 50%`, animation `typingPulse` 1.2s infinite (scale 0.6/opacity 0.4 → scale 1/opacity 1), with `150ms` and `300ms` stagger delays on dots 2 and 3. | Always cyan, never orange. |
| **Logo plaque (light)** | `display: grid; place-items: center;` · `min-height: 248px;` · `border-radius: 20px;` · `border: 1px solid rgba(17,17,17,0.08);` · logo `width: min(100%, 270px);` |  |
| **Logo plaque (dark)** | Same dimensions; `background: radial-gradient(circle at top, rgba(0,212,255,0.16), transparent 44%), #0F0F0F;` · `border-color: rgba(255,255,255,0.08);` | Cyan halo behind the orange logo. |
| **Input field** | `border-radius: 18px;` · `border: 1px solid var(--line);` · `padding: 14px 16px;` · focus: `box-shadow: 0 0 0 3px rgba(255,98,0,0.20);` and `border-color: var(--orange);` | Geist Sans 400, no inset shadow. |

For any new section, use the BEM-ish naming you see in `index.html` (`.hero-copy`, `.hero-stage`, `.section-card`, `.palette-grid`, `.prompt-card`). Don't invent a new naming scheme.

---

## 8. Logo usage

The Epifly mark is a **dynamic orange swallow / starburst** — sharp, angular, ascending flight with an energetic burst. SVG sources live in `docs/branding/logo-*.svg` (one per palette colour: `vibrant-orange`, `electric-cyan`, `deep-charcoal`, `deep-indigo`, `midnight-navy`, `warm-off-white`, `ember`, `lagoon-teal`, `solar-gold`).

Rules:
- **Default mark:** `logo-vibrant-orange.svg` (`#FF6200`).
- **On dark canvas:** keep the orange mark. Optionally surround with a cyan radial halo (see "Logo plaque (dark)" recipe).
- **On orange surfaces:** switch to `logo-warm-off-white.svg`.
- **Never distort.** Keep the original aspect ratio. Pad with whitespace — do not scale non-uniformly.
- **Wordmark pairing:** logo at `34px` height, `12px` gap, then "Epifly" in Geist Sans 700, `letter-spacing: -0.04em`.
- **Min size:** `24px` standalone, `28px` paired with wordmark.

---

## 9. Motion — psychology-led, research-backed

Epifly motion is **functional, not decorative**. Every transition has to do one of four jobs (Material Motion, Apple HIG, and Pasquale D'Silva's 2024–2026 motion-as-language work converge on this):

1. **Cause** — show the user that *their* input produced the change (button → menu, send → bubble).
2. **Hierarchy** — establish what entered, what left, what stayed.
3. **Continuity** — preserve spatial context across views (shared-element transitions, View Transitions API).
4. **Feedback** — confirm or refute a system state change (success, error, loading).

If a proposed animation does none of those four jobs, **delete it**. Idle pulses, decorative rotations, and ambient parallax are explicitly off-brand.

### 9.1 Research anchors (cite these when defending a choice)

| Principle | Implication for Epifly |
|---|---|
| **Doherty Threshold** (Doherty & Thadhani, IBM, 1982; re-validated 2020s) — responses < 400ms feel like instant productivity. | Cap all UI feedback animations at **≤ 300ms**. Anything longer needs a loading affordance. |
| **Hick's Law** | Stagger menu items at most 30ms apart — perceived as "one motion", not a parade. |
| **Fitts's Law** | Hover lifts must not move a target out from under the cursor — limit `translateY` to `−2px`, never `−8px`. |
| **Aesthetic-Usability Effect** (Kurosu & Kashimura, Norman) | Polished motion makes users tolerant of minor friction. Worth doing well; not worth fakery. |
| **Disney's 12 Principles** (adapted to UI by Pasquale D'Silva, *Transitional Interfaces*) | Specifically: **anticipation** (slight squash before launch), **follow-through** (overshoot on arrival), **arcs** (curved easing curves, not linear), **timing** (mass-appropriate durations), **slow in / slow out** (the `cubic-bezier` shape itself). |
| **Spring physics over keyframe ease** (Framer Motion / Motion One, 2024–2026 consensus) | Reach for `cubic-bezier(0.34, 1.56, 0.64, 1)` (mass-1, stiffness-260, damping-20 equivalent) for *arrivals*; `cubic-bezier(0.4, 0, 0.2, 1)` for *state changes*; `cubic-bezier(0.32, 0.72, 0, 1)` (Apple's "easeOut") for *dismissals*. |
| **Common Fate (Gestalt)** | Elements that move together belong together — stagger reveals from the same container by 30–60ms total to imply group identity, not isolation. |
| **Change blindness mitigation** | Animate position deltas > 8px. Below that, cross-fade — the eye loses motion below ~8px and an instant swap is misread as a refresh. |
| **Vestibular safety (WCAG 2.1 / 2.2)** | Honour `prefers-reduced-motion: reduce` — replace movement with opacity-only crossfades, never with `animation: none` that leaves elements invisible. |

### 9.2 Canonical motion tokens

```css
:root {
  /* Durations — every value is a Fibonacci-ish step matching Doherty + perception research */
  --d-instant: 120ms;   /* hover paint, focus ring, micro-feedback */
  --d-fast:    180ms;   /* state changes (chevron rotate, theme swap) */
  --d-base:    240ms;   /* most UI: tab switch, menu open, tooltip */
  --d-emph:    300ms;   /* arrivals, message bubbles, modal in */
  --d-slow:    480ms;   /* page / route reveal, hero scroll-in */

  /* Easings — three curves cover 95% of cases */
  --ease-ui:     cubic-bezier(0.4, 0, 0.2, 1);     /* state change — Material standard */
  --ease-out:   cubic-bezier(0.32, 0.72, 0, 1);    /* dismiss / exit — Apple-style */
  --ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1); /* arrival with overshoot */
}
```

Pick a duration **token**, never a raw number. If you reach for a fourth easing, you've left the system.

### 9.3 Pattern library

| Pattern | Recipe | Job it does |
|---|---|---|
| **Hover lift** | `transform: translateY(-2px); transition: transform var(--d-instant) var(--ease-ui), box-shadow var(--d-instant) var(--ease-ui);` Dark mode adds cyan glow `0 0 0 1px rgba(0,212,255,0.30), 0 10px 30px rgba(0,212,255,0.15)`. | Feedback (mouse here) |
| **Button press (anticipation + release)** | `:active { transform: scale(0.96); transition: transform 80ms var(--ease-ui); }`. Spring back on release via `var(--ease-spring)` at `var(--d-fast)`. | Cause (you pressed me) |
| **Message arrival (overshoot)** | `@keyframes messageIn { from { opacity:0; transform: scale(0.95) translateY(12px); } to { opacity:1; transform: none; } }` at `var(--d-emph) var(--ease-spring)`. | Hierarchy (new thing entering) |
| **Menu / popover open** | Origin-aware: `transform-origin` matches trigger position. Scale `0.96 → 1`, opacity `0 → 1`, `var(--d-base) var(--ease-ui)`. | Continuity (came from there) |
| **Sidebar / sheet slide** | `translateX(-100% → 0)` at `var(--d-emph) var(--ease-out)`. | Continuity (off-screen → on-screen) |
| **Sun ↔ Moon toggle** | Crossfade icons at `var(--d-fast) var(--ease-ui)`. Never rotate — rotation reads as "loading". | Feedback |
| **Chevron expand** | `transform: rotate(180deg)` at `var(--d-fast) var(--ease-ui)`. | Cause + state |
| **Typing indicator** | Three `6×6px` cyan dots, `1.2s` infinite, scale `0.6 → 1` + opacity `0.4 → 1`, stagger `0 / 150ms / 300ms`. **The only sanctioned looping animation** — communicates ongoing AI thought. | Feedback (system busy) |
| **Loading spinner** | Lucide `Loader2` with `animation: spin 0.9s linear infinite;`. Use only when the wait is > 400ms (Doherty); under that, no spinner — just disable the trigger. | Feedback |
| **Copy → confirm** | Swap icon `Copy` → `Check`, colour shifts to success green for 1.4s, then reverts. | Feedback (success) |
| **Reveal on scroll** | Opacity `0 → 1`, `translateY(16px → 0)`, `var(--d-slow) var(--ease-ui)`, triggered by IntersectionObserver toggling `.is-in-view` with **rootMargin: -10%** (don't fire until well into view). Stagger group children by 60ms max. | Hierarchy (entering composition) |
| **Sticky topbar settle** | When `window.scrollY > 16`, add `.is-scrolled` → fade in `box-shadow: var(--shadow)` at `var(--d-base) var(--ease-ui)`. | Continuity (page is below the rail now) |
| **Skeleton shimmer** | A *single* slow gradient sweep, `1.6s` infinite, NOT a strobing colour change. Cyan-tinted at `rgba(0,212,255,0.06)`. | Feedback (loading) |
| **Route / view transition** | Use the **View Transitions API** when available (`document.startViewTransition`) with `view-transition-name: ...` on shared elements. Fallback: opacity crossfade `var(--d-base)`. | Continuity (shared element) |
| **Error shake** | `translateX(-4px, 4px, -3px, 3px, 0)` over 220ms `var(--ease-ui)`. Single oscillation only. Never on success or info. | Feedback (failure) |

### 9.4 Stagger choreography

For lists, grids, and section reveals:

- **Item delay:** `60ms` between items.
- **Maximum visible stagger:** `300ms` total (≤ 5 items perceived as "one motion"; beyond that, treat the rest as a synchronous group).
- **Direction:** top-to-bottom for vertical lists, top-left-to-bottom-right for grids — never reverse.
- **Trigger:** `--stagger-delay: calc(var(--i) * 60ms);` on each child, with `--i` set via inline style.

### 9.5 Performance constraints

- Animate only **`transform`** and **`opacity`** in the hot path (compositor-only properties).
- **Never** animate `width`, `height`, `top`, `left`, `margin`, `padding`, `box-shadow` colour stops, `background-position` for layout. Use `transform: scale()` + a fixed-size box, or the FLIP technique.
- Promote with `will-change: transform` only **during** the animation; remove it after. Stale `will-change` declarations blow up memory on long-scrolling pages.
- Frame budget: every animation must hit 60 FPS on a 2020-era MacBook Air. If you can't, simplify.

### 9.6 Reduced motion fallback

```css
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

This collapses motion to ~instant but leaves opacity and colour transitions perceptible. Never use `animation: none` blindly — it can leave elements stuck at `opacity: 0`.

### 9.7 Hard rules

- Never animate **hue** (looks like a glitch).
- Never animate **letter-spacing** or **font-weight** (re-rasters the glyph cache).
- Never loop animations on idle elements other than the typing indicator and a single skeleton shimmer.
- Never overshoot on `:hover` (it reads as a "click confirmed" — wrong signal).
- Never spring on a dismissal — exits should feel decisive, not springy.
- Never trigger animations from `useEffect` / `onMount` without an `IntersectionObserver` or user-action gate — autoplaying motion below the fold burns frames and battery.

---

## 10. Accessibility (must pass)

- `:focus-visible` → `outline: none;` plus `box-shadow: 0 0 0 3px rgba(255,98,0,0.30);` (or `rgba(0,212,255,0.35)` on orange surfaces). Always visible against both canvases.
- Skip link slides from `top: -100%` to `0` on focus, pill-shaped, `--orange` fill, white text.
- Touch targets ≥ `44×44px`.
- Do not rely on colour alone — pair status badges with a glyph or text (`✓ Ready`, `! Warning`).
- Body copy must be `--text-soft` or `--charcoal` / `--off-white`. Orange and cyan are reserved for ≥18px display text, icons, glow halos, and decorative emphasis. Body in saturated colour fails AA contrast.
- Always pair icon-only buttons with both `aria-label` and a tooltip on hover/focus.
- Theme toggle must persist (localStorage) and respect `prefers-color-scheme` on first visit.

---

## 11. Cross-platform component sharing — `packages/ui` first

Epifly ships to **three targets** from one codebase: `apps/web` (SvelteKit web), `apps/browser-shell` (Tauri 2 desktop + iOS + Android), and embedded WebViews. Every visual or behavioural primitive must look and feel the same on all three. **The shared library is always your first choice; an app-local file is the exception you must justify.**

### 11.1 The three placement buckets

| Bucket | Where it lives | When to pick it | Example |
|---|---|---|---|
| **Shared** (default) | `packages/ui/src/lib/…` exported via the public barrel | Code that *could* run on any of the three targets. If you have to think for more than 30 seconds, the answer is "shared". | `AgentChatComposer`, `WorkspaceExplorer`, `recentsStore`, every motion primitive (`tap`, `springAnimate`, `playFlip`, `stagger`, `startViewTransition`), Lucide-icon wrappers. |
| **Mobile chrome** | `apps/browser-shell/src/lib/mobile/…` | UI patterns that have **no** desktop analogue: bottom sheets stacked on top of a drawer, screen-stack navigation, breadcrumb back-buttons, swipe-to-dismiss gestures, status-bar inset handling. | `MobileBottomSheet`, `MobileDrawer`, `MobileTopBar`, `drawerStore`, `sheetStore`, `screenStore`, `DrawerWorkspaceTree` (as a *presentation* shell over a shared store). |
| **App-local** | `apps/web/src/lib/…` **or** `apps/browser-shell/src/lib/…` | Code that imports a platform-only API: Tauri `invoke`, SvelteKit `$app/environment` / `$app/forms`, WKWebView bridges, the Rust `webview` postMessage channel, OS keychain access. | `TraceReplayCapability.svelte` (Tauri `invoke`), `apps/*/src/lib/sdk.ts` (per-app auth composition), `apps/web/src/lib/server/session.ts`. |

If a component does **not** import from a `@tauri-apps/*`, `$app/*`, `$env/*`, or app-only path, it belongs in **shared**. There are no exceptions for "it's just for the mobile drawer for now" — code routinely outlives its first call site.

### 11.2 The "headless core + presentation shell" pattern

When the same logic shows up on two surfaces with different chrome (a workspace tree on desktop sidebar vs. a workspace tree in a mobile drawer), do **not** duplicate the SDK calls and state. Instead split the component:

1. **Headless core** (lives in `packages/ui/src/lib/.../create*.svelte.ts`) — owns the SDK calls, reactive state, and mutation logic. Returns an object of `$state`/`$derived` accessors and methods. **Zero markup.**
2. **Presentation shells** — one per surface (desktop in `packages/ui`, mobile in `apps/browser-shell/src/lib/mobile/parts/`). Each renders the same store with its own chrome (sidebar rows vs. drawer rows vs. tab bar).

```ts
// ✅ Right — packages/ui/src/lib/features/createWorkspaceTreeStore.svelte.ts
export function createWorkspaceTreeStore({ sdk }: { sdk: ConusSdk }) {
  let roots = $state<WorkspaceNode[]>([]);
  let expanded = $state(new Set<string>());
  // …toggle, create, remove
  return { get roots() { return roots; }, /* … */ };
}

// packages/ui/src/lib/features/WorkspaceExplorer.svelte  — desktop chrome
// apps/browser-shell/src/lib/mobile/parts/DrawerWorkspaceTree.svelte  — mobile chrome
// Both consume createWorkspaceTreeStore.
```

Apply the same split for: chat composers, capability lists, profile menus, search results, settings panels.

### 11.3 Cross-platform tokens & motion are a contract, not a suggestion

- All three apps consume `packages/ui/src/lib/tokens.css` — **no** app-local override files.
- Motion primitives are imported from `@conusai/ui/motion` (the `tap` action's android branch reads `document.documentElement.dataset.platform`, so the *same import* renders correctly on every platform).
- Icons come from the same Lucide / Phosphor instance via `packages/ui/src/lib/icons/` (see §6) — never re-install the icon library in an app folder.
- Stores that hold *content* (recents, breadcrumbs, theme, feature flags) live in `packages/ui/src/lib/stores/`. Stores that hold *chrome state* (drawer open?, sheet stack, current mobile screen) live with the chrome.

### 11.4 The exception list (kept tiny on purpose)

These are the only situations where an app-local component is correct:

1. **Tauri-only capability** — calls `@tauri-apps/api/core invoke()`. (`TraceReplayCapability.svelte`.)
2. **SvelteKit-only primitive** — depends on `$app/environment`, `$app/forms`, `$app/state`, or a `+server.ts` runtime contract. (`apps/web/src/lib/sdk.ts`'s `browser` import.)
3. **OS-bridge integration** — keychain, biometrics, push notifications, the Rust-side recorder bridge.
4. **Per-app build env** — `import.meta.env.VITE_API_BASE` rewriting, app-specific cookie strategies.

If your component doesn't match one of these four, it belongs in `packages/ui`.

### 11.5 Imports — always use the public surface

When importing across app ↔ package boundaries, **always go through the package's public exports**, never deep paths:

```ts
// ✅ Right
import { tap, springAnimate } from "@conusai/ui/motion";
import { recentsStore } from "@conusai/ui/stores";
import { AgentChatComposer } from "@conusai/ui/features";

// ❌ Wrong — bypasses the public API, breaks on package restructure
import { tap } from "../../../packages/ui/src/lib/motion/tap.js";
import { tap } from "@conusai/ui/src/lib/motion/tap.js";
```

Subpath exports are declared in `packages/ui/package.json` — `@conusai/ui`, `@conusai/ui/motion`, `@conusai/ui/stores`, `@conusai/ui/features`, `@conusai/ui/utils`, `@conusai/ui/capabilities`, `@conusai/ui/tokens.css`, `@conusai/ui/foundry.css`. If you need to add a subpath, declare it there first — never reach into `src/`.

### 11.6 Decision checklist (run before creating any new file)

Ask in order; stop at the first **yes**:

1. Does the code import `@tauri-apps/*`, `$app/*`, `$env/*`, or an OS bridge? → app-local.
2. Is the *only* difference from an existing shared component a markup wrapper / responsive layout? → **don't** create a new component. Add a prop or a slot to the shared one, or extract a headless core (§11.2).
3. Is it a mobile chrome primitive with no desktop analogue (drawer, bottom sheet, screen stack)? → `apps/browser-shell/src/lib/mobile/`.
4. Anything else → `packages/ui/src/lib/`. **Default.**

---

## 12. File / naming conventions

- Tokens → `packages/ui/src/lib/tokens.css` (add Epifly tokens here if missing).
- Primitives → `packages/ui/src/lib/components/*` (one Svelte/React file per primitive; CSS-in-component is fine if it references the tokens).
- Section / page styles → keep co-located with the page module under `apps/*/src/lib/.../*.svelte` or equivalent.
- State classes only: `.is-open`, `.is-active`, `.is-scrolled`, `.is-in-view`, `.is-dragging`, `.is-loading`, `.is-typing`.
- Theme class on `<body>`: `data-theme="dark"` or unset for light. Never both at once.

When adding a new primitive, prefix CSS classes with the component name (e.g. `.message-bubble__avatar`, `.prompt-card__body`).

---

## 13. Self-review checklist (run before finishing)

**Brand & tokens**
- [ ] All colours reference `var(--…)` tokens — zero inline hex.
- [ ] Only Geist and Geist Mono are used.
- [ ] Radius is one of `14`, `18`, `20`, `28`, or `999`px.
- [ ] No purple, no rainbow, no Inter, no neumorphism, no system-ui fallback fonts.

**Iconography**
- [ ] **Zero emoji** in any product chrome (buttons, labels, tooltips, empty states, badges, error messages).
- [ ] All icons come from **one** approved library per surface (Lucide / Phosphor / Tabler).
- [ ] Stroke width is `1.5px` (≥24px icons) or `1.75px` (16–20px icons).
- [ ] Icons use `currentColor`, not hard-coded fills.
- [ ] Decorative icons have `aria-hidden="true"`; standalone icon buttons have `aria-label` + tooltip.
- [ ] Loading uses Lucide `Loader2` or the typing-dots recipe — never a spinning emoji.

**Motion**
- [ ] Every animation does one of the four jobs (cause / hierarchy / continuity / feedback). Cosmetic motion has been deleted.
- [ ] Durations come from the `--d-instant / --d-fast / --d-base / --d-emph / --d-slow` token scale.
- [ ] Easings come from `--ease-ui / --ease-out / --ease-spring` — no fourth curve.
- [ ] Only `transform` and `opacity` are animated in the hot path.
- [ ] `prefers-reduced-motion` fallback present and collapses safely (no stuck-invisible elements).
- [ ] No spring on `:hover` or on dismissals; no idle loops other than typing dots / single skeleton shimmer.
- [ ] Stagger ≤ 60ms per item, ≤ 300ms total.

**Surface**
- [ ] Translucent panels use `backdrop-filter: blur(18px)` over the canvas.
- [ ] Shadows use `--shadow` / `--shadow-cta` recipes (or cyan glow in dark mode).
- [ ] Component renders correctly in **both** light and dark — visually verified.

**Typography**
- [ ] Headings carry negative letter-spacing (`-0.02em` to `-0.06em`).
- [ ] Section starts with an eyebrow (orange Geist Mono uppercase) plus optional hairline.
- [ ] Reading measure capped at `56–64ch`.

**Accessibility**
- [ ] Hover and `:focus-visible` states defined for every interactive element.
- [ ] Touch targets ≥ `44×44px`.
- [ ] State not conveyed by colour alone.

**Cross-platform sharing (§11)**
- [ ] Code lives in `packages/ui/src/lib/…` unless it matches one of the four exceptions in §11.4 (Tauri / SvelteKit-only / OS bridge / per-app build env).
- [ ] Ran the §11.6 decision checklist before creating any new file.
- [ ] No SDK calls or reactive state are duplicated across desktop and mobile chrome — if the same logic appears in two places, a headless `create*Store()` was extracted to `@conusai/ui` (§11.2).
- [ ] All imports across the app ↔ package boundary use the public surface (`@conusai/ui`, `@conusai/ui/motion`, `@conusai/ui/stores`, `@conusai/ui/features`, …). No relative `../../packages/ui/src/…` paths.
- [ ] Tokens come from `packages/ui/src/lib/tokens.css` — no app-local override file.
- [ ] Motion primitives imported from `@conusai/ui/motion`, not redefined.
- [ ] Component renders correctly on **all** targets it claims to support — web, Tauri desktop, iOS, Android. Specifically: touch targets, safe-area insets, and `data-platform` branches all verified.

If any box is unchecked, fix it before returning code.

---

## 14. When the request conflicts with the system

If the user asks for something that breaks these rules (e.g. "make the hero purple", "use Inter", "add a heavy drop shadow"), **flag the conflict and propose the on-brand equivalent**. Do not silently break the brand. Examples:

- Asked for a "purple gradient hero" → offer a translucent panel over the signature orange+cyan radial canvas glow, with `--orange` for the CTA and `--cyan` for an accent halo.
- Asked to "use Inter for headings" → push back; Epifly headings are Geist 800. Geist Mono is the only secondary face.
- Asked for "drop shadows on cards" → use the documented `var(--shadow)` soft elevation, or the dark-mode cyan glow recipe. No black box shadows.
- Asked for "fully rounded blob buttons" → propose pill (`999px`) for chips/topbar, or `18px` for primary CTAs. No values between `28px` and `999px`.
- Asked for "✨ Generate" / "🚀 Get started" / emoji-in-button → swap to the Lucide equivalent (`Sparkles`, `Rocket`) keeping the same label text. Note the substitution in your reply.
- Asked for "make it more lively / add a pulsing glow / make the logo rotate" → push back; ambient idle motion is forbidden. Offer instead a meaningful arrival animation, a hover lift, or a typing indicator scoped to actual AI activity.
- Asked to "just put this component in `apps/web/src/lib/`" or "this is mobile-only for now" → run the §11.6 checklist. If none of the four exceptions match, place it in `packages/ui/src/lib/` and reply explaining the decision. Components routinely outlive their first call site.
- Asked to "copy this from the web app to the mobile app" → refuse and propose either a shared component in `packages/ui` or a headless-core + presentation-shell split (§11.2). Duplication of SDK calls or reactive state across `apps/web` and `apps/browser-shell` is never the right answer.
- Asked to import via a deep relative path (`../../../packages/ui/src/lib/...`) → use the subpath export (`@conusai/ui/motion`, etc.); if the subpath doesn't exist yet, add it to `packages/ui/package.json` exports first.

Stay polite, state the rule, and offer the on-brand alternative.

---

## Reference

**Brand**
- Brand kit: [docs/branding/branding.md](../../docs/branding/branding.md)
- Reference desktop layout: [docs/branding/index.html](../../docs/branding/index.html)
- Reference mobile layout: [docs/branding/mobile.html](../../docs/branding/mobile.html)
- Logo variants: `docs/branding/logo-*.svg` (vibrant-orange is the default mark)

**Shared library (`@conusai/ui`)** — always import from these subpaths, not deep paths:
- `@conusai/ui` — root barrel: components, theme provider, capability registry.
- `@conusai/ui/features` — composed features: `AgentChatComposer`, `AgentChatStream`, `WorkspaceExplorer`, `ToolCallCard`, workspace dialogs.
- `@conusai/ui/motion` — `tap`, `springAnimate`, `playFlip` / `recordRect`, `stagger`, `startViewTransition`.
- `@conusai/ui/stores` — `recentsStore`, `breadcrumbsStore`, theme store, feature flags, toasts, mode store.
- `@conusai/ui/utils` — `prefersReducedMotion`, `autoGrow`, `renderMarkdown`, `LiveAnnouncer`.
- `@conusai/ui/capabilities` — capability renderer registry.
- `@conusai/ui/tokens.css` and `@conusai/ui/foundry.css` — global CSS.

**Apps**
- `apps/web` — SvelteKit web (cookie auth, same-origin fetch).
- `apps/browser-shell` — Tauri 2 desktop + iOS + Android (device-token auth, `X-Session-Token`, `VITE_API_BASE` rewriter, `src/lib/mobile/` for chrome-only patterns).
