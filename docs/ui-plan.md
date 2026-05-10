# ConusAI Mobile-First UI — Implementation Plan

> **Spec source:** [docs/tasks/ui-task.md](tasks/ui-task.md)
> **Strict guidelines:** Premium White + Teal Ember (2026.05.11) — embedded as §0 below. Non-negotiable.
> **Design system:** [docs/ui-design.md](ui-design.md) — tokens in [`packages/ui/src/lib/tokens.css`](../packages/ui/src/lib/tokens.css) and [`packages/ui/src/lib/foundry.css`](../packages/ui/src/lib/foundry.css).
> **Target host app:** [apps/browser-shell](../apps/browser-shell) (Tauri + SvelteKit, `adapter-static`, ships to macOS / Windows / iOS / Android).
> **Reuse policy:** Do **not** fork existing chat / workspace logic. The mobile shell is a *layout layer* over the existing `@conusai/ui` features and `@conusai/sdk` clients.
> **Icon policy:** Inline SVG only. **No emoji anywhere in the UI.** No icon library (no `lucide-svelte`).
> **One shell everywhere:** `MobileShell` is the **canonical** experience on iPhone, Android, macOS, and Windows. Desktop simply widens the content column and keeps the same chrome — no separate desktop layout. See §0.10 and §4.

---

## 0. Strict Guidelines (locked — apply to every step)

These rules are **non-negotiable**. Every file must pass lint + visual audit before merge. Any deviation requires written approval from the product lead.

### 0.1 Color Tokens — zero hex literals in component code

All colors **must** come from the existing tokens in [`packages/ui/src/lib/tokens.css`](../packages/ui/src/lib/tokens.css) and [`packages/ui/src/lib/foundry.css`](../packages/ui/src/lib/foundry.css). The token *definitions* below are the source of truth; component code uses only `var(--token)` references.

**Light (Paper — default premium white)**

| Token | Value | Role |
|---|---|---|
| `--paper`     | `#FAF9F5` | main background, top bar, composer dock |
| `--paper-2`   | `#F0ECE6` | AI bubbles, raised surfaces, drawer, composer inner |
| `--paper-3`   | `#E6E1D9` | hover states, subtle dividers |
| `--ink`       | `#14110D` | primary text, user-bubble text on accent |
| `--ink-2`     | `#3A332B` | secondary text, timestamps |
| `--ink-3`     | `#6E6357` | muted labels, placeholders |
| `--rule`      | `#D6CAB0` | hairline borders (1 px) |
| `--seam`      | `#C2B391` | stronger dividers (drawer, sheets) |

**Forge (Dark)**

| Token | Value |
|---|---|
| `--paper`   | `#100E0B` |
| `--paper-2` | `#181612` |
| `--paper-3` | `#211E18` |
| `--ink`     | `#F4EEE3` |
| `--ink-2`   | `#C8BFAE` |
| `--ink-3`   | `#8A8174` |
| `--rule`    | `#2A251E` |
| `--seam`    | `#3A3328` |

**Shared accents (both themes)**

| Token | Value | Role |
|---|---|---|
| `--ember`      | `#80CDC6` | **sole accent** — streaming dots, send button, active states, focus rings |
| `--ember-2`    | `#5AADA6` | pressed / hover accent |
| `--ember-soft` | `rgba(128,205,198,0.10)` | context chips, active rows |
| `--ember-glow` | `rgba(128,205,198,0.28)` | cursor glow, avatar highlights |
| `--success`    | `#1A7F4B` | semantic only |
| `--danger`     | `#B32400` | semantic only |
| `--steel`      | `#5C6B7A` | neutral chrome (rare) |

**Rules.** Never hard-code any hex in `apps/browser-shell/src/lib/mobile/**`. Theme toggle swaps via `data-theme="forge"` on `<html>` (already wired in `ThemeProvider`). `--ember` is the **only** accent — no additional hues, ever.

### 0.2 Typography (2026 premium agent standard)

```css
--font-sans:    "Inter",        system-ui, -apple-system, sans-serif;  /* body, UI, chat text */
--font-display: "Space Grotesk", sans-serif;                            /* headings, greeting, top bar */
--font-mono:    "Space Mono",   ui-monospace, monospace;                /* paths, capability names, code */
```

- Body / chat text: **17 px** (`--font-sans`, line-height 1.5).
- Greeting headline: **32 px**, `--font-display`, `letter-spacing: -1px`, `line-height: 1.05`.
- Top-bar title: **18 px**, `--font-display`, `letter-spacing: -0.4px`.
- Drawer section labels: **11 px**, `--font-mono`, uppercase, `letter-spacing: 0.08em`, `color: var(--ink-3)`.
- All text uses `--ink` / `--ink-2` / `--ink-3` only.

### 0.3 Spacing & radii

| Spacing |  | Radius |  |
|---|---|---|---|
| `--s-1` | 4 px  | `--r-md` | 14 px (buttons, chips, small cards) |
| `--s-2` | 8 px  | `--r-lg` | 24 px (bubbles, sheets, composer) |
| `--s-3` | 12 px |          |                                      |
| `--s-4` | 16 px |          |                                      |
| `--s-5` | 20 px |          |                                      |
| `--s-6` | 24 px |          |                                      |
| `--s-8` | 32 px |          |                                      |

### 0.4 Layout & navigation (strict)

- **Zero bottom navigation bar — ever.**
- **Single top bar**, 48 px + safe-area-top: `left = menu/back`, `centre = breadcrumb or screen title`, `right = one contextual icon only`.
- **Left drawer is the only navigation surface.** It is opened/closed **manually** via the top-bar menu button (plus `Escape` and backdrop click). No swipe-to-open. No persistent peek. 84 % width, max 320 px.
- Drawer contents (single scroll container, top → bottom):
  1. **User profile header** — avatar, name, plan; tap → profile sheet (login / logout / settings).
  2. **Workspace navigation** — full hierarchical tree with inline create actions (new folder, new file, new conversation). This *is* the workspace browser; there is no separate workspace screen.
  3. **Recent chats** — below the workspace tree, mono section label, list of recent threads. Tap loads in chat.
- Default launch: **ChatScreen** (empty centred greeting).
- Capabilities and Artifacts are reached **exclusively** from the drawer via the section links above the workspace tree.
- Safe areas: `padding-top: max(var(--s-3), env(safe-area-inset-top))`, `padding-bottom: max(var(--s-2), env(safe-area-inset-bottom))`.
- Composer always sticky-bottom on the chat screen with `border-top: 1px solid var(--rule)`.

### 0.5 Premium agent chat UX (ChatScreen)

- **Empty state**: perfectly centred (flex column, items-centre, justify-centre). Sigil 68 × 68 px on `--paper-2` with breathing animation; Space Grotesk greeting "Good morning, {name}"; subtext 17 px `--ink-2`. Composer centred just below.
- **Suggestion scaffolding (psychology — close the motivation/ability gap):** *up to 4* ghost-style suggestion chips render **only on the empty state**, in a single horizontal scroll row immediately below the composer. Chips: `--paper` background, `1 px solid var(--rule)`, `--font-sans` 14 px `var(--ink-2)`, `border-radius: var(--r-md)`, padding `var(--s-2) var(--s-3)`, hover/focus → `border-color: var(--ember); color: var(--ink)`. Source: `sdk.suggestions.forUser()` if available, otherwise a curated static set scoped per workspace. Tap = prefill composer + focus. Chips **stagger-fade-in** 40 ms apart on mount and **fade out together with the greeting on first send**. They never reappear in the same session. Rationale: Hick's Law (≤ 4 options), goal-gradient (lowers activation energy), aesthetic-usability (still feels minimal because chips are ghost-weight).
- **First send**: empty state + chips fade out (300 ms), composer animates from centre → docked bottom (spring), messages container fades + slides up into the freed space. Reduced-motion → 80 ms cross-fade.
- **User bubbles**: `background: var(--ember)`, `color: var(--ink)`, right-aligned, `border-radius: var(--r-lg) var(--r-lg) 8px var(--r-lg)`.
- **AI bubbles**: `background: var(--paper-2)`, `color: var(--ink)`, left-aligned, `border-radius: var(--r-lg) var(--r-lg) var(--r-lg) 8px`.
- **Composer**: flat `--paper-2` inner, no heavy shadow, 44 × 44 px icon + send buttons, 17 px text.
- **Thinking state**: three pulsing dots `--ember` inside a `--paper-2` pill.
- **Context chip**: `--ember-soft` background, left-aligned above composer, clearable via inline `×` icon.
- **Attachment button** → bottom sheet (camera / files / workspace picker).

### 0.6 Motion — premium native feel (no Framer Motion)

The "Framer-Motion-grade" feel is achieved with **Svelte built-in transitions + a tiny `spring()` store + `View Transitions API` where supported** — zero new dependencies. Animations honour platform conventions and `prefers-reduced-motion`.

**Tokens** (define in [`packages/ui/src/lib/tokens.css`](../packages/ui/src/lib/tokens.css) if missing):

```css
--dur-1: 120ms;  --dur-2: 180ms;  --dur-3: 240ms;  --dur-4: 320ms;  --dur-5: 480ms;
--ease-out:      cubic-bezier(0.22, 1, 0.36, 1);    /* iOS-like ease-out-quint */
--ease-in-out:   cubic-bezier(0.65, 0, 0.35, 1);    /* symmetrical, for crossfades */
--ease-spring:   cubic-bezier(0.34, 1.56, 0.64, 1); /* mild overshoot, sheets / FAB */
--ease-standard: cubic-bezier(0.2, 0, 0, 1);        /* Material 3 emphasized */
--ease-emphasized-decel: cubic-bezier(0.05, 0.7, 0.1, 1.0); /* M3 emphasized decelerate */
```

**Per-platform motion mapping** (one set of tokens, platform-conditional via `navigator.userAgent` detected once in `MobileShell.onMount` and exposed on `<html data-platform="ios|android|macos|windows">`):

| Surface | iOS / macOS | Android | Windows |
|---|---|---|---|
| Drawer open / close | `transform: translateX` + `--ease-out` 240 ms (iOS-quint), backdrop fades 200 ms | `--ease-standard` 220 ms, backdrop fades 180 ms | 200 ms `--ease-out` |
| Bottom sheet | spring (Svelte `spring`, stiffness 0.18, damping 0.85) → mimics UISheet detents, rubber-band overscroll | `transform` + `--ease-emphasized-decel` 240 ms | `--ease-out` 200 ms |
| Screen cross-fade | View Transitions API where supported, else 200 ms `fade` | View Transitions API or 200 ms `fade + slide y:8` | View Transitions API or 200 ms `fade` |
| Empty → active chat | composer flies via Svelte `spring` (stiffness 0.16, damping 0.78) — Apple-rubbery on iOS, tighter damping (0.9) on Android | — | — |
| Tap feedback | scale 0.98 over 80 ms `--ease-out` | ripple via inline radial-gradient `--ember-soft` 280 ms | scale 0.98 over 80 ms |
| Focus ring | 80 ms fade-in, no scale | same | same |

**Animation catalogue (the polish that separates premium from generic).** Implement these globally via `motion/` helpers; they are the small details users feel but never name.

| # | Moment | Effect | Notes |
|---|---|---|---|
| 1 | App launch | Sigil `sigil-enter` (scale 0.92 → 1, opacity 0 → 1, 480 ms `--ease-emphasized-decel`) followed by `sigil-breathe` (3 % scale loop, 4 s) | Already in `foundry.css` — wire into ChatScreen mount. |
| 2 | Greeting reveal | Word-by-word **stagger-fade-up** (each word `opacity 0 → 1`, `translateY 6px → 0`, 40 ms apart, 240 ms each) | Anchors attention; classic Linear / Arc move. |
| 3 | Suggestion chips appear | Stagger-fade-in 40 ms apart, `translateY 8px → 0`, 220 ms `--ease-out` | Second wave after greeting settles. |
| 4 | First send (the hero moment) | Flip-style transition: composer position recorded → state changes → spring tween from old → new bounding box (`stiffness: 0.18, damping: 0.82`); messages container fades + slides up 24 px in parallel | Implement as `motion/flip.ts` (FLIP technique, no library). |
| 5 | User bubble enter | `scale 0.96 → 1` + `opacity 0 → 1`, origin bottom-right, 220 ms `--ease-spring` | Reads as a satisfying "pop in place". |
| 6 | AI bubble enter | `opacity 0 → 1` + `translateY 8px → 0`, 240 ms `--ease-out` | Calmer than user bubble — asymmetry signals "AI is thinking, not reacting". |
| 7 | Token streaming | Each new chunk wrapped in span with `opacity 0 → 1` over 60 ms; subtle `--ember-glow` cursor pulse at the live edge (1 s sine loop) | Mimics Claude/ChatGPT's signature live-typing feel. |
| 8 | Thinking pill | Three dots, staggered `translateY 0 → -2px → 0` 600 ms loop, 100 ms phase offset | Visible patience signal — Doherty-threshold compliant. |
| 9 | Drawer open | Backdrop `opacity 0 → 1` 200 ms, panel `translateX(-100%) → 0` with platform curve, **content children stagger-fade-up** 30 ms apart inside the panel | Makes the drawer feel "composed", not just slid in. |
| 10 | Drawer close | Reverse, but 80 % duration (faster exit than entry — Material guidance) | |
| 11 | Tree expand/collapse | `transition:slide` + chevron rotates 0 → 90° via `transform` 180 ms `--ease-out` | |
| 12 | Bottom sheet open | Spring detents on iOS; `--ease-emphasized-decel` 240 ms on Android; backdrop fades 180 ms | |
| 13 | Bottom sheet drag | Live `transform: translateY` follows finger; release → spring to nearest detent or dismiss | iOS-grade; uses `motion/spring.ts`. |
| 14 | Tap feedback | iOS/macOS/Win: `scale 0.98` 80 ms in / 120 ms out. Android: ripple via inline radial-gradient on `--ember-soft`, 280 ms expand + 200 ms fade | |
| 15 | Focus ring | `box-shadow` 0 → `0 0 0 3px var(--ember-soft)` 80 ms; never animates layout | |
| 16 | Send button success | `scale 1 → 0.92 → 1` 220 ms `--ease-spring` on send commit | Tactile confirmation. |
| 17 | Error toast | Slide down from top + tiny shake (`translateX -2 → 2 → 0` 80 ms × 3) | Shake reserved for real errors only — habituation guard. |
| 18 | Workspace row activation | `--ember-soft` background fades in 120 ms, left rail `--ember` slides in via `transform: scaleY(0 → 1)` 180 ms from top | |
| 19 | Theme switch | Use **View Transitions API** (`document.startViewTransition`) for a global cross-fade between paper ↔ forge — single line of code, zero jank | Progressive enhancement; fallback = instant swap. |
| 20 | Pull-to-refresh (chat list) | iOS: rubber-band `translateY` follows finger, sigil rotates proportionally; release → spring back + `sigil-breathe` flash | Optional, only if backend supports refetch. |

**Reduced motion**: every transition above clamps to an 80 ms opacity-only fade; springs disabled (snap to final value); ripples disabled; word-stagger collapses to single 80 ms fade.

**Implementation primitives** (all in `apps/browser-shell/src/lib/mobile/motion/`, no external deps):
- `spring.ts` — wraps Svelte `spring()` for transform/opacity tweens.
- `flip.ts` — First/Last/Invert/Play helper for the empty → active composer transition (#4).
- `stagger.ts` — Svelte action that delays children by index × N ms.
- `viewTransition.ts` — feature-detected `document.startViewTransition` wrapper for #3 (screen cross-fade) and #19 (theme switch).
- `tap.ts` — Svelte action attaching scale-tap or ripple based on `data-platform`.

### 0.7 Iconography

- **Inline SVG only**, defined at the call site. 24 × 24 viewbox standard, `stroke: currentColor`, `stroke-width: 1.75`, `fill: none`.
- **No emoji** in any user-visible string (greetings, empty states, errors, drawer labels, tooltips, sheet titles). Lint rule: `apps/browser-shell/src/lib/mobile/**` is grepped for emoji code points and PR-blocked on hits.
- A canonical inline-SVG cheat-sheet lives at the top of `MobileShell.svelte` as commented snippets so contributors copy-paste rather than reinvent.
- Reuse glyphs already present in [`apps/web/src/routes/+page.svelte`](../apps/web/src/routes/+page.svelte) (menu, back chevron, send, paper-clip, mic).

### 0.8 Accessibility & quality gates

- Minimum tap target **44 × 44 px**.
- Visible focus ring: `box-shadow: 0 0 0 3px var(--ember-soft)`.
- `aria-label` on every icon-only button.
- Live region for streaming announcements (reuse `LiveAnnouncer` from `@conusai/ui`).
- Lighthouse mobile **≥ 95 / 95 / 95** on Performance, Accessibility, Best Practices.
- Zero hex literals in component code: `grep -EnR '#[0-9a-fA-F]{3,8}' apps/browser-shell/src/lib/mobile` returns nothing.
- Zero emoji: emoji-range grep returns nothing.
- Every component **≤ 200 LOC**, single concern, typed with TypeScript.
- No new npm dependencies.

### 0.9 Verification checklist (must pass before every PR)

- [ ] Uses **only** the listed tokens (no hex literals).
- [ ] Premium white `--paper` (`#FAF9F5`) background on light theme.
- [ ] Teal `--ember` (`#80CDC6`) is the **sole** accent.
- [ ] No bottom navigation bar.
- [ ] No emoji anywhere in the mobile shell.
- [ ] Empty chat perfectly centred; first send animates cleanly to docked composer.
- [ ] Up to 4 ghost suggestion chips on empty state; vanish with greeting on first send; never return in-session.
- [ ] Drawer is the only navigation surface; opens/closes only via top-bar button (manual).
- [ ] Drawer hosts profile · workspace tree (with create actions) · recent chats — in that order.
- [ ] Safe areas + reduced motion respected on iOS, Android, macOS, Windows.
- [ ] **Same `MobileShell` renders on iPhone, Android, macOS, and Windows** (no desktop fork).
- [ ] Self-hosted woff2 fonts; identical rendering across WebKit / Blink / WebView2.
- [ ] All 20 catalogue animations (§0.6) implemented and pass reduced-motion fallback.
- [ ] Playwright mobile viewport (390 × 844) passes; desktop viewport (1280 × 800) passes the same suite.
- [ ] Visual regression snapshots green on iOS sim, Android emu, macOS, Windows.

---

### 0.10 One shell on every platform (consistency lock)

The **same `MobileShell.svelte` is the only shell**, regardless of viewport or OS:

- Mobile (iOS / Android, ≤ 640 px): full-bleed.
- Desktop (macOS / Windows, > 640 px): same shell, content column clamped to `max-width: 760px` (chat) / `880px` (capabilities, artifacts), centred. Drawer becomes a permanent, *manually toggleable* left rail (still hidden by default; same open animation, no auto-pin).
- Top bar, drawer, sheets, bubble shapes, motion catalogue — **identical** in markup and tokens. Only the content `max-width` and the platform-tagged motion curve differ.
- The legacy `AppShell + TabStrip + RecorderControls` is **removed** from the active route in Step 1; kept in the repo behind a `legacy/` folder for one release for reference, then deleted.

This collapses the maintenance surface to a single layout tree and makes user behaviour identical across devices — critical for muscle-memory and onboarding research.

### 0.11 Font hosting (cross-platform consistency)

- Self-host **woff2** files for Inter, Space Grotesk, Space Mono in [`packages/ui/src/lib/assets/fonts/`](../packages/ui/src/lib/assets/fonts/).
- `@font-face` declarations in [`packages/ui/src/lib/foundry.css`](../packages/ui/src/lib/foundry.css) with `font-display: swap` and `unicode-range` subsets (latin + latin-ext).
- Preload the two most-used weights in [apps/browser-shell/src/app.html](../apps/browser-shell/src/app.html): `<link rel="preload" as="font" type="font/woff2" crossorigin>`.
- Never rely on Google Fonts at runtime — Tauri webviews on Windows + offline iOS sim must render identically.

### 0.12 Adding a new screen (extensibility recipe)

Locked, repeatable workflow — should take ≤ 30 minutes per new screen:

1. Create `apps/browser-shell/src/lib/mobile/screens/FooScreen.svelte` (≤ 200 LOC, tokens only, no emoji).
2. Add `'foo'` to the `Screen` union in `stores/screen.svelte.ts`.
3. Add a row in `MobileDrawer.svelte` secondary-links section (single inline-SVG icon + label).
4. Add `#/foo` case in the `hashchange` listener inside `MobileShell.svelte`.
5. Add a Playwright case in `mobile-shell.spec.ts` covering: drawer → tap row → screen renders → back closes correctly.
6. Run audits: hex grep, emoji grep, `pnpm test`, Lighthouse.

No other file touched. This recipe is the contract for "easy to extend".

---

## 1. Architecture

```
apps/browser-shell/src/lib/mobile/
├── MobileShell.svelte                # root: top bar, screen host, drawer host, sheet host
├── motion/
│   ├── spring.ts                     # tiny wrapper over Svelte spring store
│   ├── flip.ts                       # FLIP technique for empty→active composer hero transition
│   ├── stagger.ts                    # Svelte action: stagger children by index × N ms
│   ├── viewTransition.ts             # feature-detected document.startViewTransition wrapper
│   └── tap.ts                        # platform-aware scale-tap / ripple action
├── platform/
│   └── detect.ts                     # sets <html data-platform="ios|android|macos|windows">
├── stores/
│   ├── screen.svelte.ts              # active screen + per-screen nav stack
│   ├── breadcrumbs.svelte.ts         # current workspace path
│   ├── drawer.svelte.ts              # left drawer open/close (manual only)
│   ├── sheet.svelte.ts               # bottom sheet stack
│   └── recents.svelte.ts             # recent threads (rehydrates from localStorage)
├── chrome/
│   ├── MobileTopBar.svelte           # menu/back · breadcrumb · contextual action
│   ├── MobileDrawer.svelte           # left modal drawer (only nav surface)
│   └── MobileBottomSheet.svelte      # generic spring-based bottom sheet
├── screens/
│   ├── ChatScreen.svelte             # primary screen (empty → active states)
│   ├── CapabilitiesScreen.svelte     # semantic search + cards
│   └── ArtifactsScreen.svelte        # files list with previews
└── parts/
    ├── DrawerProfileHeader.svelte    # avatar + name + plan; opens profile sheet
    ├── DrawerWorkspaceTree.svelte    # hierarchical tree + inline create actions
    ├── DrawerRecentChats.svelte      # recent threads list
    ├── WorkspaceTreeRow.svelte       # one node row (folder | file | conversation)
    ├── WorkspaceCreateMenu.svelte    # +-button popover: New folder / New file / New chat
    ├── ProfileSheet.svelte           # login / logout / theme / app version
    ├── Breadcrumbs.svelte
    ├── ContextChip.svelte
    ├── SuggestionChips.svelte        # ghost-style empty-state chips (Hick's-Law-bounded ≤ 4)
    ├── CapabilityRow.svelte
    ├── CapabilityDetailSheet.svelte
    ├── AttachmentSheet.svelte
    └── ArtifactRow.svelte
```

### 1.1 Navigation model (drawer-only, manual)

- The shell has a single persistent surface: the **top bar**. Left = back chevron when a stack is non-empty, otherwise menu icon (toggles drawer). Centre = `<Breadcrumbs />` or screen title. Right = one contextual icon (e.g. paper-clip count on chat, search on capabilities).
- The **left drawer** is the *only* navigation entry point and is opened/closed **only manually** by the top-bar menu button (plus `Escape` and backdrop click). No swipe-to-open. No persistent rail.
- Drawer order, top → bottom:
  1. `DrawerProfileHeader` — avatar + name + plan; tap → `ProfileSheet` (login / logout / theme / version).
  2. Section divider, `WORKSPACE` mono label, with a trailing `+` button → `WorkspaceCreateMenu` (New folder / New file / New chat).
  3. `DrawerWorkspaceTree` — hierarchical, expand/collapse via `transition:slide`. Tap folder = expand; long-press folder = create-into menu; tap file/conversation = load in `ChatScreen` and close drawer.
  4. Section divider, `RECENT` mono label.
  5. `DrawerRecentChats` — last 20 threads from `recents.svelte.ts` (rehydrated from localStorage). Tap = `chatStream.loadThread(id)` + close drawer.
  6. Section divider, secondary links (Capabilities, Artifacts) — these route to their respective screens.
- Default screen on launch is **Chat**, empty state. Workspace context is empty until the user picks a node from the drawer tree.

### 1.2 Routing

No new SvelteKit routes. Mobile is gated by the existing `(max-width: 640px)` rune in [apps/browser-shell/src/routes/+layout.svelte](../apps/browser-shell/src/routes/+layout.svelte#L11). Deep links use URL hash:

- `#/chat?node=<NodeId>&thread=<ThreadId>` (default)
- `#/capabilities?q=<query>`
- `#/artifacts?node=<NodeId>`

A single `hashchange` listener in `MobileShell.svelte` syncs `screenStore`. Tauri deep-link plugin maps `conusai://...` → `location.hash`.

### 1.3 State sources reused

- Chat: `createChatStream(sdk, { streamFn: tauriStreamFn })` (already in [`apps/browser-shell/src/routes/+page.svelte`](../apps/browser-shell/src/routes/+page.svelte#L15)).
- Workspace tree: `sdk.workspaces.tree(parentId)`, `sdk.workspaces.create(...)`.
- Capabilities: `sdk.capabilities.search(q)` + `sdk.capabilities.list()`.
- Files: `sdk.workspaces.upload(file)`; `sdk.workspaces.getContent(id)` for previews.
- Auth: existing `auth` store from `@conusai/sdk` for profile + logout.

---

## 2. Step-by-Step Plan

Each step is independently shippable, has a clear deliverable, and ends with a verification command. **Do not start step N+1 until step N is verified.**

### Step 1 — Unified shell switch + platform tagging + font hosting

**Goal:** make `MobileShell` the single shell on every platform; tag platform for motion; lock font rendering.

- Edit [apps/browser-shell/src/routes/+layout.svelte](../apps/browser-shell/src/routes/+layout.svelte): **always** render `<MobileShell />`; move legacy `AppShell + TabStrip + RecorderControls` to `apps/browser-shell/src/lib/legacy/` (kept one release, then deleted).
- Add `viewport-fit=cover` and `interactive-widget=resizes-content` to [apps/browser-shell/src/app.html](../apps/browser-shell/src/app.html) `<meta name="viewport">`. Add `apple-mobile-web-app-capable`.
- Add `<link rel="preload" as="font" type="font/woff2" crossorigin>` for Inter 400 + 600 and Space Grotesk 600 in `app.html`.
- Self-host woff2 fonts in [`packages/ui/src/lib/assets/fonts/`](../packages/ui/src/lib/assets/fonts/) and declare `@font-face` (with `font-display: swap` + `unicode-range`) in [`packages/ui/src/lib/foundry.css`](../packages/ui/src/lib/foundry.css). No Google Fonts at runtime.
- Create `apps/browser-shell/src/lib/mobile/platform/detect.ts`: `onMount` sets `document.documentElement.dataset.platform = 'ios' | 'android' | 'macos' | 'windows'`.
- Create `apps/browser-shell/src/lib/mobile/MobileShell.svelte` with `background: var(--paper); color: var(--ink)`, safe-area paddings, and a content-width clamp: `max-width: 760px` on `(min-width: 641px)`.

**Verify:** `pnpm --filter browser-shell dev`; same shell renders at 390 × 844, 768 × 1024, and 1280 × 800; `data-platform` correct on each OS; fonts identical across WebKit / Blink / WebView2 (visual diff).

---

### Step 2 — Motion primitives + stores

**Goal:** native-feeling animations without new deps; single source of truth for nav state.

- `motion/spring.ts` — wraps Svelte `spring()` for transform/opacity tweens; honours `prefers-reduced-motion`. Per-platform default tunings.
- `motion/flip.ts` — FLIP helper for the empty → active composer hero transition (catalogue #4).
- `motion/stagger.ts` — Svelte action delaying children by `index × N ms`; powers greeting word-stagger (#2), suggestion chips (#3), drawer content (#9).
- `motion/viewTransition.ts` — feature-detected `document.startViewTransition` wrapper; powers screen cross-fade and theme switch (#19). Falls back to fade.
- `motion/tap.ts` — Svelte action: `scale-tap` on iOS/macOS/Windows, ripple on Android, gated by `data-platform`.
- `stores/screen.svelte.ts` — `$state` for `active: 'chat' | 'capabilities' | 'artifacts'` (default `'chat'`) + per-screen stack. Methods: `setActive`, `push`, `pop`, `canGoBack`.
- `stores/breadcrumbs.svelte.ts` — derived from current `WorkspaceNode`.
- `stores/drawer.svelte.ts` — `open`, `toggle`, `close`. **Manual only**, no auto-open.
- `stores/sheet.svelte.ts` — stack-based `push/pop`.
- `stores/recents.svelte.ts` — last 20 thread IDs in localStorage; `add(id)`, `list()`.

All stores SSR-safe (no `window` at module scope). All motion helpers respect `prefers-reduced-motion`.

**Verify:** Vitest covers stores (push/pop/active, drawer toggle, recents cap). Storybook-style scratch route `/motion-debug` cycles through every animation in the §0.6 catalogue with a reduced-motion toggle.

---

### Step 3 — Chrome primitives (top bar, drawer shell, bottom sheet)

**Goal:** ship the persistent chrome with full a11y + native motion. **No bottom bar.**

- `MobileTopBar.svelte` — height 48 px + safe-area top, `border-bottom: 1px solid var(--rule)`, `background: var(--paper)`. Slots: left (menu/back, 44 × 44 px), centre (`<Breadcrumbs />` or screen title in `--font-display` 18 px), right (single contextual icon button).
- `MobileDrawer.svelte` — fixed-left, 84 % width max 320 px, `background: var(--paper-2)`, `border-right: 1px solid var(--seam)`. `transform: translateX(-100%) → 0` over `var(--dur-3)` with platform-scoped easing (see §0.6). Backdrop `color-mix(in srgb, var(--ink) 40%, transparent)`. Trap focus, `Escape` closes, click-outside closes. **No swipe-open gesture.**
- `MobileBottomSheet.svelte` — anchored bottom, `background: var(--paper)`, `border-top: 1px solid var(--rule)`, `border-radius: var(--r-lg) var(--r-lg) 0 0`. Drag handle 40 × 4 px `var(--rule)`. iOS = spring detents (peek 25vh / half 50vh / full 90vh) via `motion/spring.ts`; Android/Windows = `--ease-standard` snap. Swipe-down + backdrop dismiss.

Inline-SVG cheat-sheet lives at the top of `MobileShell.svelte`.

**Verify:** dev-only `/mobile-debug` route renders all three; manual check on 390 × 844 — targets ≥ 44 px, focus visible, animations < 320 ms, reduced-motion collapses to 80 ms fade.

---

### Step 4 — `ChatScreen.svelte` (primary / default screen)

**Goal:** Claude-grade chat with persistent composer. Default landing screen.

**Two states, one screen — empty → active transition is the centrepiece.**

- **Empty state (no messages):** flex-column centre/centre.
  - Sigil 68 × 68 px (`favicon` asset on `--paper-2`, `border-radius: var(--r-lg)`) with `sigil-enter` (#1) then `sigil-breathe` from [`foundry.css`](../packages/ui/src/lib/foundry.css#L385).
  - Greeting "Good morning, {name}" — `--font-display` 32 px, `letter-spacing: -1px`, `line-height: 1.05`. Each word renders inside its own `<span>` and uses `motion/stagger.ts` (#2) — 40 ms delay per word, `opacity 0 → 1`, `translateY 6px → 0`, 240 ms `--ease-out`.
  - Subtext 17 px `var(--ink-2)`, fades in after greeting completes.
  - Composer centred just below.
  - **`SuggestionChips.svelte`** below composer: up to 4 ghost-style chips (see §0.5), staggered in 40 ms apart (#3) after subtext settles. Sourced via `sdk.suggestions.forUser()` with a curated static fallback. Tap = prefill composer + focus. **Hidden permanently after the first send within the session** (in-memory flag).
  - Nothing else on screen besides the top bar.
- **First send transition (catalogue #4 — the hero moment):**
  - Record composer bounding box via `motion/flip.ts`.
  - Greeting + subtext + chips fade out together (`transition:fade={{ duration: 280 }}`).
  - State flips → composer re-renders at docked bottom.
  - FLIP plays the spring tween from old → new box (`stiffness: 0.18, damping: 0.82`), giving a true Apple-grade rubbery dock.
  - Messages container fades + slides up 24 px in parallel (`transition:fly={{ y: 24, duration: 320, easing: cubicOut }}`).
  - On `prefers-reduced-motion` all three collapse to 80 ms opacity fade; FLIP disabled.
- **Active state:** existing `AgentChatStream` from `@conusai/ui/features`. Override `max-width: 100%` on `(max-width: 640px)` via wrapper class.
  - **User bubbles:** `background: var(--ember)`, `color: var(--ink)`, right-aligned, `border-radius: var(--r-lg) var(--r-lg) 8px var(--r-lg)`, padding `var(--s-3) var(--s-4)`. Enter animation #5 (scale-pop from bottom-right origin).
  - **AI bubbles:** `background: var(--paper-2)`, `color: var(--ink)`, left-aligned, `border-radius: var(--r-lg) var(--r-lg) var(--r-lg) 8px`, padding `var(--s-3) var(--s-4)`. Enter animation #6 (calm fade-up).
  - **Streaming token feed (#7):** new chunks fade in 60 ms each; live edge has a `--ember-glow` cursor that pulses on a 1 s sine loop. Disabled on reduced-motion.
  - **Thinking pill (#8):** three dots, 100 ms-staggered `translateY` bounce, 600 ms loop, paused on reduced-motion.
- **Persistent composer dock:** existing `AgentChatComposer` wrapped in `<div class="mobile-composer-dock">`:
  - `position: sticky; bottom: env(safe-area-inset-bottom)`.
  - `background: var(--paper)`, `border-top: 1px solid var(--rule)`.
  - Inner field on `var(--paper-2)`, `border-radius: var(--r-lg)`, padding `var(--s-3) var(--s-4)`, 17 px text.
  - Hosts `ContextChip` above (workspace path + clear `×`) — only when a workspace node is selected from the drawer.
  - Send button 44 × 44 px, `background: var(--ember)`, hover `var(--ember-2)`, focus-ring `var(--ember-soft)`.
- Send pipeline reuses `chatStream.send(prompt, { workspaceNodeId, attachmentIds })`. Send button success animation #16 fires on commit.
- Tauri streaming via `streamChatTauri` ([apps/browser-shell/src/lib/tauri-stream.ts](../apps/browser-shell/src/lib/tauri-stream.ts)).
- Attachment button → `sheet.push({ key: 'attachment' })` → `AttachmentSheet` (Step 7).
- Voice button feature-flagged off until backend ready.

**Verify:** Playwright at 390 × 844 *and* 1280 × 800: launch → empty centred state, greeting word-stagger, chips appear, composer centred. Type + send → FLIP transition lands cleanly, composer docked, user bubble pops, AI bubble fades up, streaming cursor pulses, thinking dots before first token. Reduced-motion replaces every animation with 80 ms fade. Chips never reappear in the same session.

---

### Step 5 — Drawer content (profile · workspace tree + create · recents)

**Goal:** the drawer is the entire navigation, file management, and account surface.

#### 5.1 `DrawerProfileHeader.svelte`

- Top of drawer, padding `var(--s-4)`.
- Avatar 40 × 40 px circle (`background: var(--paper-3)`, initials in `--font-display` 16 px if no image).
- Name (`--font-sans` 15 px `var(--ink)`) + plan (`--font-mono` 11 px `var(--ink-3)`).
- Tap entire row → `sheet.push({ key: 'profile' })` → `ProfileSheet` (login if signed-out; theme switcher; logout; app version).
- Hairline `border-bottom: 1px solid var(--rule)` below.

#### 5.2 `DrawerWorkspaceTree.svelte` + `WorkspaceTreeRow.svelte` + `WorkspaceCreateMenu.svelte`

- Section header: `WORKSPACE` (`--font-mono` 11 px uppercase `var(--ink-3)`), trailing `+` icon button (24 px) → `WorkspaceCreateMenu` popover (anchored to `+`):
  - **New folder** → inline rename row appears at root, commit on `Enter`, `sdk.workspaces.create({ kind: 'folder', name, parent_id: currentParent })`.
  - **New file** → file picker → `sdk.workspaces.upload(file, parent_id)`.
  - **New chat** → `chatStream.newThread()` + `setActive('chat')` + close drawer.
- Tree rows (`WorkspaceTreeRow.svelte`):
  - Indent 16 px per depth level.
  - Leading 20 px inline SVG (folder | file | chat-bubble).
  - Label `--font-sans` 15 px `var(--ink)`.
  - Mono path hint `--font-mono` 11 px `var(--ink-3)` on hover only.
  - Active row: `background: var(--ember-soft)`, left rail 2 px `var(--ember)`.
  - Tap folder → expand/collapse via `transition:slide={{ duration: 200 }}`; lazily loads children via `sdk.workspaces.tree(node.id)`.
  - Tap file/conversation → `chatStream.loadThread(thread_id)` (or sets context for non-conversation), `setActive('chat')`, close drawer.
  - Long-press folder → `WorkspaceCreateMenu` anchored to row, scoped to that parent.
  - Long-press any node → action sheet (rename, delete, share). Delete confirmed in sheet.
- Loading: 4 skeleton rows shimmering `--paper-2` → `--paper-3`, paused on reduced-motion.
- Error: inline banner `color: var(--danger)` + retry.

#### 5.3 `DrawerRecentChats.svelte`

- Section header `RECENT` (same mono style).
- Up to 20 rows from `recents.svelte.ts`. Each row: title (15 px `--ink`), timestamp (mono 11 px `--ink-3`).
- Tap → `chatStream.loadThread(id)` + `setActive('chat')` + close drawer.
- Empty: muted "No recent chats" (`--ink-3`).

#### 5.4 Secondary links (bottom of drawer)

- Hairline divider.
- Two rows (44 px each, single inline-SVG icon + label):
  - **Capabilities** → `setActive('capabilities')` + close drawer.
  - **Artifacts**    → `setActive('artifacts')` + close drawer.

**Verify:** drawer opens only via top-bar menu (no other tap target opens it); `Escape` and backdrop close it; create-folder commits; expand/collapse animates < 200 ms; recent chat tap loads thread and closes drawer; long-press surfaces actions; profile sheet logout works.

---

### Step 6 — `CapabilitiesScreen.svelte`

**Goal:** filtered, workspace-aware capability list. Reachable only from drawer.

- Sticky search input (`--font-mono` caret, debounce 200 ms → `sdk.capabilities.search(q, 20)`). `background: var(--paper-2)`, `border: 1px solid var(--rule)`, `border-radius: var(--r-md)`.
- Default (empty query): `sdk.capabilities.list()` grouped by namespace; sections collapsible via `transition:slide`.
- `CapabilityRow.svelte`: name (`--font-sans` 15 px semibold `var(--ink)`), description (13 px `var(--ink-2)`, 2-line clamp), namespace pill (`--font-mono` 11 px, `background: var(--ember-soft)`, `color: var(--ember)`).
- Tap → `sheet.push({ key: 'capability-detail', props: { id } })` → `CapabilityDetailSheet` shows full description + tools list + "Invoke in current workspace" primary button. Invoke = `setActive('chat')` + prefill composer with `/${cap.name} `.
- Empty filtered: "No matching capabilities." — no decorative art, no emoji.

**Verify:** type a query; results update under 300 ms; detail sheet opens; invoke lands focus in chat composer.

---

### Step 7 — `AttachmentSheet.svelte`

**Goal:** add attachments without leaving the chat screen.

- Three rows in a bottom sheet:
  - **Camera** — only renders if Tauri camera plugin available.
  - **Photos / Files** — hidden `<input type="file" multiple>` with mime hints.
  - **From workspace** — mini tree picker reusing the `DrawerWorkspaceTree` data hook.
- Uploads via `sdk.workspaces.upload`; resulting `Attachment` pushed to composer attachments rune.
- Each row: 56 px height, hairline divider, label `--font-sans` 15 px, leading 24 px inline SVG.

**Verify:** upload a file → chip appears in composer → send → message body includes `attachment_ids`.

---

### Step 8 — `ArtifactsScreen.svelte`

**Goal:** browse generated artifacts. Single-column list (no grid — minimal).

- `ArtifactRow.svelte`: leading 32 px file-type inline SVG, name (`--font-sans` 15 px), meta (`--font-mono` 11 px `--ink-3` — size · timestamp).
- Filter by current `workspaceNode.tags` derived from breadcrumbs.
- Tap row → bottom sheet preview (reuse `ArtifactPreview` from `@conusai/ui`).
- Long-press → action sheet (download, share, delete).

**Verify:** open artifact → preview renders → share copies link.

---

### Step 9 — Motion catalogue completion, theming, polish

- Wire all 20 animations from §0.6 catalogue. Each ships with a reduced-motion fallback test.
- Screen cross-fade via `motion/viewTransition.ts` (#3); falls back to `transition:fade` when API unsupported.
- Theme switch via `document.startViewTransition` (#19) — paper ↔ forge cross-fades the entire app in one frame.
- Tap feedback: `motion/tap.ts` action attached globally to `[role=button]` and `<button>` (#14).
- Drawer + sheet motion finalised per platform (§0.6 / catalogue #9–#13).
- iOS rubber-band scroll lock on chrome (`overscroll-behavior: contain`).
- Lighthouse mobile + desktop audit ≥ 95 across the board.
- **Audits** (CI-blocking):
  - `grep -EnR '#[0-9a-fA-F]{3,8}' apps/browser-shell/src/lib/mobile` → empty.
  - Emoji grep (`grep -PnR '[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]' apps/browser-shell/src/lib/mobile`) → empty.
  - Reduced-motion Playwright run: confirms every animation collapses to ≤ 80 ms opacity fade.

---

### Step 10 — Tauri / native wiring

- Update [apps/browser-shell/src-tauri/tauri.conf.json](../apps/browser-shell/src-tauri/tauri.conf.json) `app.windows` mobile defaults; `withGlobalTauri = true` on iOS so `__TAURI_INTERNALS__` exists.
- Register `tauri-plugin-deep-link` and map `conusai://` → `location.hash` in `MobileShell.onMount`.
- Wire Android hardware back button → `screenStore.pop()` (falls through to drawer-close, then no-op).
- macOS/Windows: titlebar inset respected; drawer animation curve switches to platform value via `data-platform`.
- Smoke test: `pnpm tauri ios dev` (iPhone 16 sim), `pnpm tauri android dev` (Pixel 7), `pnpm tauri dev` on macOS + Windows narrow window.

---

### Step 11 — E2E + visual regression

- Playwright spec [`apps/web/e2e/mobile-shell.spec.ts`](../apps/web/e2e/mobile-shell.spec.ts) runs at **three viewports** (390 × 844 mobile, 768 × 1024 tablet, 1280 × 800 desktop) and on **two motion settings** (default + `prefers-reduced-motion`):
  1. Launch → ChatScreen empty: sigil enter, greeting word-stagger, chips fade in, centred composer; no other chrome.
  2. Type + send first message → greeting + chips fade out, FLIP composer dock, user bubble pops, AI bubble fades up, streaming cursor visible.
  3. Suggestion chips: tap one → composer prefilled + focused; chips never reappear in the same session.
  4. Confirm drawer is the only nav: no swipe-open works; only top-bar menu button opens it; `Escape` and backdrop close it; drawer content stagger-fades-in.
  5. Drawer shows profile · workspace tree · recent chats · capabilities · artifacts in order.
  6. Create a new folder from drawer `+` button; row appears in tree with slide-in.
  7. Tap a recent chat → drawer closes (faster exit), thread loads in chat.
  8. Open Capabilities → search → detail sheet → invoke → composer prefilled.
  9. Theme switch via `ProfileSheet`: View Transitions cross-fade plays in one frame.
  10. Keyboard accessibility: Tab order, `Escape` closes drawer/sheet, focus returns to top-bar menu.
  11. Reduced-motion run: every animation ≤ 80 ms fade; no transforms.
  12. No emoji and no hex literals in built CSS for `lib/mobile/`.
- Visual snapshots saved under `test-results/mobile-shell-visual/{ios,android,macos,windows}/` for diff on each PR.

---

## 3. Definition of Done

- All 11 steps merged. Mobile is the default at ≤ 640 px; no feature flag.
- **Default launch = ChatScreen empty state**: greeting + sigil + centred composer, nothing else on screen.
- **First-send transition lands cleanly** with platform-appropriate spring/ease; reduced-motion → 80 ms fade.
- **Drawer is the only nav surface**, opened/closed **manually** from the top-bar menu button. It hosts profile, workspace tree (with create/rename/delete), recent chats, and links to Capabilities + Artifacts.
- **Zero bottom navigation bar.**
- **Zero emoji** anywhere in the mobile shell.
- **Zero hex literals** in `apps/browser-shell/src/lib/mobile/**`.
- **Zero new dependencies** (no Framer Motion, no icon library).
- Native motion mapped per platform (iOS/macOS soft springs, Android Material emphasized, Windows linear-ish) via a single `data-platform` attribute and `motion/spring.ts`.
- Lighthouse mobile ≥ 95 / 95 / 95 on `pnpm preview` build.
- Playwright `mobile-shell.spec.ts` green on CI; visual snapshots reviewed.
- All new code ≤ 200 LOC per file, single concern, typed with TypeScript.

---

## 4. Cross-Platform Behaviour

**One shell, every platform.** `MobileShell.svelte` is the *only* layout. Per-platform deltas live exclusively in motion curves and tap feedback (via `data-platform`) and the desktop content-width clamp.

| Target | Layout | Motion / native notes |
|---|---|---|
| iOS (Tauri) | `MobileShell`, full-bleed | iOS-quint easing, soft springs (UISheet detents), scale-tap. `streamChatTauri` IPC bypasses WKWebView SSE buffering. Safe-area insets active. |
| Android (Tauri) | `MobileShell`, full-bleed | Material 3 emphasized-decelerate easing, ripple tap feedback, hardware back → `screenStore.pop()` then drawer-close. |
| macOS (Tauri desktop) | `MobileShell`, content clamped to 760 / 880 px, drawer toggleable rail | Same iOS-style easing; titlebar inset respected; scale-tap. |
| Windows (Tauri desktop) | same as macOS | Slightly faster `--ease-out`; scale-tap (no ripple); WebView2 font-rendering verified. |
| Web ([apps/web](../apps/web)) | responsive | Desktop layout retained for marketing routes; promote `MobileShell` into `packages/ui` for full web parity in a follow-up. |

---

## 5. Brand Asset Audit

Current state of [packages/ui/src/lib/assets/](../packages/ui/src/lib/assets):

| Asset | Used? | Where |
|---|---|---|
| `images/favicon.png` | Yes | [apps/web/src/routes/+layout.svelte](../apps/web/src/routes/+layout.svelte#L4), greeting sigil in [apps/web/src/routes/+page.svelte](../apps/web/src/routes/+page.svelte#L10) |
| `images/conusai-logo-darkmode.png` | Yes | [apps/web/src/routes/login/+page.svelte](../apps/web/src/routes/login/+page.svelte#L3) |
| `images/conusai-logo-lightmode.png` | Listed in [packages/ui/scripts/assets-verify.js](../packages/ui/scripts/assets-verify.js#L11), no runtime import yet | — |
| `icons/icons.svg` | Listed in `assets-verify.js`, no runtime import | mobile shell uses inline SVGs only |

**Action items folded into this plan:**
- **Step 4** — Greeting (empty Chat state) reuses the existing `favicon` sigil with `sigil-enter + sigil-breathe` from [`packages/ui/src/lib/foundry.css`](../packages/ui/src/lib/foundry.css#L385).
- **Step 5** — `DrawerProfileHeader` uses the user's avatar (or Space-Grotesk initials on `--paper-3`); no logo in the drawer chrome (premium minimal).
- **`icons/icons.svg`** — keep as fallback sprite; not used in mobile shell. If still unused after Step 11, delete it and update `assets-verify.js`.

---

## 6. Token Naming Decision

**Decision: keep the existing token names.** Do not rename `--paper`, `--ink`, `--rule`, `--seam`, `--ember`, `--steel`, `--success`, `--danger` as part of this plan.

**Rationale.**
- The names form a single coherent **print-shop / forge metaphor** that already pervades the codebase (`paper.css` light theme, `foundry.css` dark theme, `tokens.css`, the `Foundry` design language documented in [docs/ui-design.md](ui-design.md)). The metaphor *is* the brand voice — replacing it with generic semantics would erase a deliberate design signature for zero user-visible benefit.
- Cohesion is hard-won. Every existing component, doc, and test reference assumes these names. A mass rename is high-risk, low-value, and would create merge conflicts across in-flight PRs without changing a single pixel.

| Token | Metaphor | Role |
|---|---|---|
| `--paper`, `--paper-2`, `--paper-3` | sheets stacked on a desk | canvas → surface → raised |
| `--ink`, `--ink-2`, `--ink-3` | ink darkness | primary → secondary → muted text |
| `--rule` | a printer's hairline rule | 1 px borders / dividers |
| `--seam` | binding seam between sheets | structural separators |
| `--ember` (+ `-2`, `-soft`, `-glow`) | the single warm accent in a cool press | active / streaming / focus |
| `--steel` | cold press metal | neutral chrome |
| `--success`, `--danger` | universal status | semantic only |

**Optional, non-breaking improvement.** If onboarding friction with the metaphor ever becomes a real complaint, add a thin **semantic alias layer** in [`packages/ui/src/lib/tokens.css`](../packages/ui/src/lib/tokens.css) — aliases only, no rename, no churn:

```css
:root {
  --bg-canvas:  var(--paper);
  --bg-surface: var(--paper-2);
  --bg-raised:  var(--paper-3);
  --fg-primary: var(--ink);
  --fg-muted:   var(--ink-3);
  --border:     var(--rule);
  --accent:     var(--ember);
}
```

New code may use either layer; existing code stays untouched. **Do not introduce these aliases pre-emptively** — only add them if and when a concrete onboarding pain point is reported.

---

## 7. Out of Scope

- Split-view / multi-pane layouts (single-column on every viewport).
- Voice transcription backend (UI button feature-flagged off).
- Offline mode / local DB sync (existing `RealtimeService` WS reused as-is).
- Native push notifications.
- **Any bottom navigation bar / footer chrome.**
- **Any new colour token, hex literal, emoji, or icon dependency.**
- **Framer Motion, Motion One, GSAP, or any external animation library.**
- **Any divergent desktop shell** — `MobileShell` is canonical everywhere.
