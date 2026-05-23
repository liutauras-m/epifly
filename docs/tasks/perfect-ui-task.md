> **Status:** Stack recommendations superseded by [`docs/ui-plan.md`](../ui-plan.md) (2026-05-23).
> The reference screenshots, per-screen UX critique, and visual review below remain authoritative — the **stack** recommendation (Tailwind v4 / shadcn-svelte) does not. Pixel-pass against the screenshots using the in-repo Foundry system (`packages/ui` + `tokens.css` / `foundry.css`), not shadcn primitives. See [`docs/ui-plan.md`](../ui-plan.md) §0.6 for the reconciliation rationale.

**✅ UI Review (from your attached screenshots)**

Your app is a clean, modern AI chat/workspace interface (“ConusAI / Workshop”):

- **Desktop** (persistent left sidebar + centered main chat):
  - Fixed sidebar: “WORKSPACE” + search + empty state + “RECENT” list (chats, Capabilities, Artifacts) + user footer (“John Smith ENTERPRISE”).
  - Top bar with hamburger (mobile trigger), “Workshop”, icons.
  - Main area: logo + greeting (“Good evening, John.”), message input with attachment, suggestion chips, chat bubbles (AI + user).

- **Mobile** (iOS 18.4 style on iPhone 16 Pro):
  - Top bar: “ConusAI” + hamburger + chat icon.
  - Sidebar becomes a full-height slide-in drawer (from left) showing workspace, folders (“root”), recent, sign-out.
  - Chat view is full-screen with bottom input.

**Goal**: Replicate this **exactly** with **mobile-first** responsive design using the in-repo Foundry system (`packages/ui` + `tokens.css` / `foundry.css`) in Tauri v2 + SvelteKit. ~~_(original recommendation: Tailwind v4 + shadcn-svelte — superseded by [`docs/ui-plan.md`](../ui-plan.md); see the banner at the top of this file.)_~~

### Research Summary (Reddit, X, Professional Svelte Community – May 2026)

I checked recent discussions (Reddit r/sveltejs, r/tauri, r/rust; X posts since 2025; GitHub issues, Tauri docs, shadcn-svelte repo):

- **Consensus winner**: **Tauri v2 + SvelteKit + Svelte 5 + Tailwind CSS v4 + shadcn-svelte** is the #1 recommended stack for exactly this use-case (desktop + mobile apps with responsive sidebar/chat UI). Multiple production boilerplates and YouTube talks confirm it works great for iOS/Android/macOS/Windows.
- **shadcn-svelte sidebar** is explicitly praised for mobile → desktop transition (built-in `Sidebar.Provider`, collapsible, and easy pairing with `Sheet`/`Drawer` on mobile).
- **Tailwind v4** (released Jan 2025) is fully supported and the default in new shadcn-svelte projects — faster, smaller, better CSS variables.
- **Best practices** mentioned repeatedly:
  - Mobile-first Tailwind (`sm:`, `md:`, etc.).
  - Use shadcn-svelte’s `IsMobile` hook + `Sidebar` constants for width.
  - Tauri platform detection (`@tauri-apps/api/os` or window API) for subtle HIG tweaks (iOS safe-areas, macOS titlebar, etc.).
  - Disable SSR in SvelteKit (`+layout.ts` with `ssr = false`).
  - Path aliases in `components.json`, `vite.config.ts`, `tsconfig.json` for Tauri compatibility.
  - Boilerplates like `tauri2-svelte5-shadcn` and `cnblocks` (150+ shadcn blocks with Tailwind v4) are heavily recommended for speed.
- Common pitfalls avoided: correct path aliases in Tauri, using `Sheet` for mobile drawer instead of forcing desktop sidebar.

This stack is battle-tested in 2025–2026 community projects.

### Newest Official Documentation (follow these exactly)

| Resource | URL | Why it matters |
|----------|-----|----------------|
| **shadcn-svelte** (main) | https://shadcn-svelte.com/ | Latest CLI, components, Tailwind v4 support |
| **Sidebar component** (exact match for your UI) | https://shadcn-svelte.com/docs/components/sidebar | Full docs + mobile examples, constants, collapsible |
| **Blocks / Examples** | https://shadcn-svelte.com/blocks | Ready sidebar-01, sidebar-02, chat-like layouts |
| **Tailwind CSS v4** | https://tailwindcss.com/docs | Official v4 config & utilities |
| **Tauri v2 + SvelteKit** | https://v2.tauri.app/start/frontend/sveltekit/ | Official guide (SSR disable, build config) |
| **Svelte 5 / SvelteKit** | https://kit.svelte.dev/ | Runes, latest patterns |

### Detailed Implementation Plan (step-by-step for AI / you to follow)

#### Phase 1: Project Setup (Newest Stack)

> ❌ **DO NOT EXECUTE the commands below.** This block is the *original* stack recommendation, superseded by [`docs/ui-plan.md`](../ui-plan.md). The repo already exists; the in-repo Foundry system (`packages/ui` + `tokens.css` / `foundry.css`) is the canonical stack. Block kept as historical context for the decision, not as executable instructions.

```bash
# (historical — see banner at top of file; do not run)
# 1. Create Tauri + SvelteKit app
npm create tauri-app@latest my-conusai -- --template sveltekit

# 2. Enter project
cd my-conusai

# 3. Install Tailwind v4 + shadcn-svelte (newest)
npx shadcn-svelte@latest init   # choose "new-york" style, Tailwind v4, zinc/slate base

# 4. Add core components you need
npx shadcn-svelte@latest add sidebar sheet drawer button card input avatar badge separator scroll-area
```

**Important config files to double-check** (Tauri compatibility):
- `components.json` → use `$lib/...` aliases
- `tailwind.config.ts` → Tailwind v4 format
- `vite.config.ts` → keep Tauri dev server settings
- `src/app.css` → import Tailwind + shadcn globals
- `src/routes/+layout.ts` → `export const ssr = false;`

#### Phase 2: Responsive Layout (Mobile-First)
1. **Root layout** (`src/routes/+layout.svelte`):
   - Wrap everything in `<Sidebar.Provider>` (from shadcn).
   - Use Tailwind responsive: `flex flex-col md:flex-row h-screen`.

2. **Sidebar behavior**:
   - **Desktop** (`md:` and up): Persistent left sidebar (`<Sidebar.Root>` + `Sidebar.Content`).
   - **Mobile** (`< md:`): Hidden by default + hamburger → `<Sheet>` (or `<Drawer>`) that slides in the exact sidebar content.
   - Use `const isMobile = new IsMobile();` (shadcn hook) or Tailwind `hidden md:block`.

3. **Exact components to match your screenshots**:
   - Workspace header + search → `Sidebar.Header` + `Input`
   - Recent list → `Sidebar.Group` + `Sidebar.Item` (with chat icons)
   - User footer → `Sidebar.Footer`
   - Main chat area → full-width flex with centered greeting + message bubbles + input
   - Suggestion chips → `Button` variants
   - Mobile drawer → `Sheet` with same sidebar content (no duplication — extract to reusable component)

4. **Tailwind mobile-first classes** you’ll use heavily:
   ```svelte
   <div class="flex h-screen flex-col md:flex-row">
     <!-- sidebar hidden on mobile, shown on md+ -->
     <aside class="hidden md:flex w-64 flex-col ...">
     <!-- sheet for mobile -->
     <Sheet>
   ```

5. **iOS/macOS/Android/Windows polish**:
   - Add `padding: env(safe-area-inset-top)` + Tailwind safe-area utilities for iOS notch.
   - Tauri window API for titlebar (macOS traffic lights).
   - Platform-specific CSS variables (detect once in layout).

#### Phase 3: Chat UI Implementation
- Reusable `MessageBubble` component (AI left, user right with orange accent like your screenshots).
- Input with attachment icon + send.
- Greeting screen (logo + “Good evening, John.” + chips).
- Scrollable chat area (`ScrollArea` from shadcn).

#### Phase 4: Testing & Polish
- Run: `tauri dev`, `tauri android dev`, `tauri ios dev`.
- Test responsive in browser dev tools + real simulators.
- Add theme toggle (shadcn has it built-in).
- Use `cnblocks` repo (https://sv-blocks.vercel.app) for extra chat/sidebar blocks if you want faster copy-paste.

This plan gets you **pixel-perfect** to your screenshots in a single responsive codebase.

Would you like me to:
- Generate the full folder structure + key component code snippets right now?
- Provide the exact `+layout.svelte` and sidebar implementation code?
- Or point you to the best ready-made Tauri + shadcn-svelte boilerplate that already has 90% of this?

Just say the word and I’ll drop the code. 🚀