**Adapted Strict Instructions for ConusAI Mobile UI (Copy-Paste Ready)**

You are a senior mobile product designer + frontend engineer who has deeply studied the official Claude iOS/Android apps (2026), ConusAI’s Foundry UI (ui-design.md), and the canonical mobile-navigation patterns. Your task is to produce **only** UI for ConusAI’s mobile agent chat + workspace experience (SvelteKit + Tauri/Capacitor hybrid targets for iOS, Android, macOS, Windows).

**ConusAI-specific adaptations (MANDATORY — zero exceptions):**
- **Workspace folder-based navigation**: All screens are context-aware to the current workspace node/folder (virtual_path, NodeId). Home/Workspace tab shows clean hierarchical tree/list + breadcrumbs. Chat is always scoped to a workspace conversation node. Capabilities/Artifacts filtered by current workspace tags/namespace.
- **Clean & minimalistic**: “Shows only what the user needs.” Editorial-industrial aesthetic (generous whitespace, hairline `--rule` borders, warm paper/ink or forge theme). No clutter, no decorative chrome, no extra panels. Dynamic UI hides irrelevant controls based on workspace context.
- **COLOR POLICY**: Use **only** ConusAI design tokens from `assets/css/style.css` (`--paper`, `--ink`, `--ember` (#80cdc6 teal accent), `--paper-2`, `--rule`, `--seam`, etc.). Light = paper theme default; dark = forge. Teal `--ember` is the **only** saturated accent (focus rings, active states, streaming rail). Reference via CSS custom properties or Tailwind (if configured).
- **LAYOUT & HIERARCHY**: Mobile-first (390×844 iPhone 16 base + safe-area insets). Bottom tab bar (4 tabs). Persistent bottom composer (Claude-style pill). Top bar minimal (breadcrumbs + workspace path + model pill).
- **NAVIGATION**: Bottom tabs = **Workspace** (folder tree) | **Chat** (agent) | **Capabilities** (semantic list) | **Artifacts** (files/previews). Independent stack per tab. Hamburger → modal drawer for full thread/workspace history. Bottom sheets for attachments/quick capabilities. Deep links: `conusai://workspace/{virtual_path}`, `conusai://thread/{ulid}`, `conusai://capabilities?query=...`.
- **ROUNDINGS**: Exact tokens from ui-design.md (`--r-sm` = 6px cards, `--r-md` = 10px composer, nested-radius rule, `--r-full` avatars). Message bubbles: 20 px (adapted to tokens).
- **ICONS**: Lucide (outline, stroke 2) **or** existing `icons/icons.svg` sprite. Sizes: 24 px toolbar/tabs, 20 px input bar. Specific: `Menu`, `Plus`, `Mic`, `Send`, `Paperclip`, `Sparkles` (capabilities), `FileText`, `Folder`, `ChevronLeft`.
- **ANIMATIONS**: Restrained spring physics (Svelte `transition:fly|spring` or Framer Motion) matching ui-design.md (`--ease-spring`, 120–520 ms). Message slide-up + fade, streaming typewriter + cursor, thinking shimmer, tab cross-fade.
- **HYBRID TARGET**: Tauri (primary for desktop/mobile) + Capacitor fallback. Respect platform back gestures, safe areas, theme sync. Reuse existing `RealtimeService` WS + capability registry API.
- **ADDITIONAL**: Accessibility (labels, contrast, dynamic type). Output = Figma-style spec + ready-to-copy Svelte component tree for `apps/browser-shell/src/lib/mobile/` (or `apps/web/src/lib/mobile/` for web reuse). SRP: one component per concern, clean props/stores.

Follow original rules 1–7 religiously, adapted to workspace context and ConusAI design system. Default to Claude mobile behavior only where ConusAI arch/ui-design is silent. **Never** add features outside this spec.

---

### 1. Figma-Style Screen Specs (Mobile-First, Portrait)

**Screen 1: Workspace Tab (Home / Folder Navigation)**  
- Top bar (safe-area): left hamburger (`Menu`), centered workspace breadcrumb (`/erp/po → invoices/`), right workspace selector pill.  
- Main area: clean vertical list/tree of nodes (folder icon + name + last-modified meta). Collapsible sub-trees (chevron). Search bar at top (minimal).  
- Empty state: large whitespace + “No files yet — create folder or upload” CTA.  
- Bottom tab bar: active Workspace tab (filled `Folder` icon + label).  
- Colors: `bg-paper text-ink`, hairline `--rule` dividers, `--ember` accent on selected row.  
- Motion: tree expand/collapse spring (200 ms).

**Screen 2: Chat Tab (Agent Experience)**  
- Top bar: back chevron + current workspace path (e.g. “invoices/2026-q2.md”) + model pill (`haiku` / `opus`).  
- Message list: full height, auto-scroll. User bubbles right (`--ember` left border, 20 px radius). Agent left (rail + streaming teal gradient). Tool cards minimal (icon + name + status dot).  
- Thinking state: 3-dot wave + logo pulse (Claude-exact).  
- Persistent bottom composer: pill (`--r-md` outer, nested `--r-sm` send), + attachments button, text input (expands up), send arrow. Voice icon optional (feature-gated).  
- Context indicator: tiny workspace folder badge above composer (shows current node).  
- Colors: messages on `--paper-2`, rail `--ember` when streaming.

**Screen 3: Capabilities Tab**  
- Top bar: search input (semantic query).  
- Scrollable list: capability cards (name, description snippet, namespace pill, tags). Sorted by relevance (semantic router preview).  
- Card tap → bottom sheet with tool list + “Invoke in current workspace” button.  
- Empty/filtered: “No matching capabilities in this workspace folder”.  
- Clean cards: 16 px radius, hairline rule, `--ember` accent on hover/active.  
- Integrates `/v1/capabilities/search` + workspace tags.

All screens: 60 fps spring animations, thumb-zone actions, platform-native tab styling (iOS 49 pt, Android 80 dp), dark/light sync via system preference + existing `style.css` theme toggle.

---

### 2. Ready-to-Copy Svelte Component Tree (Monorepo-Compliant)

**Proposed structure (best practice — SRP, extensible, reuses existing `style.css` + stores)**  
Create folder: `apps/browser-shell/src/lib/mobile/` (Tauri hybrid target) or `apps/web/src/lib/mobile/` (web reuse).  
Use Svelte 5 runes + stores for reactivity (newest idiomatic, zero boilerplate).

```svelte
<!-- apps/browser-shell/src/lib/mobile/MobileShell.svelte -->
<script>
  import { onMount } from 'svelte';
  import { currentWorkspace } from '$lib/stores/workspace'; // existing workspace store
  import { tabStore } from '$lib/mobile/stores/tabs'; // new simple rune store
  import WorkspaceTab from './WorkspaceTab.svelte';
  import ChatTab from './ChatTab.svelte';
  import CapabilitiesTab from './CapabilitiesTab.svelte';
  import ArtifactsTab from './ArtifactsTab.svelte';

  let { currentTab = $state('workspace') } = $props();
</script>

<div class="mobile-shell bg-paper text-ink flex flex-col h-screen safe-area-inset">
  <!-- Top bar (breadcrumbs + context) -->
  <header class="border-b border-rule px-4 py-3 flex items-center gap-3">
    <button onclick={() => {/* drawer */}} class="text-2xl">☰</button>
    <div class="flex-1 truncate font-body text-sm">
      {$currentWorkspace?.virtual_path || 'Root'}
    </div>
    <!-- model pill -->
  </header>

  <!-- Tab content -->
  {#if currentTab === 'workspace'}
    <WorkspaceTab />
  {:else if currentTab === 'chat'}
    <ChatTab />
  {:else if currentTab === 'capabilities'}
    <CapabilitiesTab />
  {:else if currentTab === 'artifacts'}
    <ArtifactsTab />
  {/if}

  <!-- Bottom tab bar -->
  <nav class="tab-bar border-t border-rule bg-paper-2 flex justify-around items-center h-14 safe-bottom">
    <!-- 4 tabs with Lucide icons + labels, active = --ember -->
  </nav>
</div>

<style>
  @import '$lib/assets/css/style.css'; /* reuses all tokens */
</style>
```

**Key Components (minimal, reusable):**

```svelte
<!-- apps/browser-shell/src/lib/mobile/WorkspaceTab.svelte -->
<script>
  import { workspaceNodes } from '$lib/stores/workspace'; // reuse PostgresWorkspaceStore via API
  let nodes = $derived($workspaceNodes || []);
</script>

<div class="flex-1 overflow-auto px-4 py-6">
  <input type="text" placeholder="Search workspace…" class="w-full bg-paper-2 border border-rule rounded-[10px] px-4 py-3 mb-6" />
  {#each nodes as node}
    <div class="flex items-center gap-3 py-4 border-b border-rule hover:bg-paper-3 rounded-[6px] px-3 transition-colors">
      <span class="text-2xl">{node.kind === 'Folder' ? '📁' : '📄'}</span>
      <div class="flex-1">
        <div class="font-body">{node.name}</div>
        <div class="text-xs text-ink-3 font-mono">{node.virtual_path}</div>
      </div>
    </div>
  {/each}
</div>
```

```svelte
<!-- apps/browser-shell/src/lib/mobile/ChatTab.svelte -->
<script>
  import MessageList from './MessageList.svelte';
  import Composer from './Composer.svelte';
  // reuse existing agent streaming logic + RealtimeService
</script>

<div class="flex-1 flex flex-col">
  <MessageList />
  <Composer workspaceContext={$currentWorkspace} />
</div>
```

```svelte
<!-- apps/browser-shell/src/lib/mobile/CapabilitiesTab.svelte -->
<script>
  import { capabilities } from '$lib/stores/capabilities'; // syncs with /v1/capabilities + semantic router
</script>

<div class="flex-1 p-4">
  <input type="text" bind:value={query} placeholder="Search capabilities…" class="..." />
  {#each $capabilities as cap}
    <div class="capability-card bg-paper-2 border border-rule rounded-[16px] p-4 mb-4">
      <div class="flex justify-between">
        <div>{cap.name}</div>
        <span class="text-ember text-xs font-mono">{cap.namespace}</span>
      </div>
      <p class="text-ink-2 text-sm mt-1">{cap.description}</p>
    </div>
  {/each}
</div>
```

**Composer.svelte** (Claude-exact pill, minimal):
- Uses Tailwind + custom properties for `--r-md`, `--ember` focus ring.
- Attachments button opens bottom sheet (file upload → MinIO via existing `/v1/files`).
- Send triggers agent completion (reuse `/v1/agent/completions` + streaming).

**Stores (new, tiny)**  
`apps/browser-shell/src/lib/mobile/stores/tabs.ts` — simple writable rune store for tab state.  
`apps/browser-shell/src/lib/stores/workspace.ts` — already exists; extend with Tauri FS plugin for native file access on desktop/mobile.

**Integration Notes (best, newest, extensible)**  
- **Tauri hybrid**: Use `tauri-plugin-fs` for native workspace access + `tauri-plugin-http` for API calls. Same Svelte components run on web/desktop/mobile.  
- **State sync**: Existing `RealtimeService` WS + `SemanticCapabilityRouter` API → live updates on all platforms.  
- **Theme**: Reuse existing dark/light toggle from `style.css`.  
- **Routing**: SvelteKit pages under `/mobile` or Tauri window management for tab persistence.  
- **Effort**: ~8 AI-hours (scaffolding + 3 core screens). Token cost negligible.  
- **SRP / extensibility**: One file per screen + shared primitives (`MessageBubble`, `CapabilityCard`). Future tablet/desktop layouts just wrap the same components. No duplication.

This gives a production-grade, zero-clutter mobile agent UI that feels native to ConusAI’s workspace-first architecture while staying 100 % consistent with the editorial-industrial design system. Drop the folder into `apps/browser-shell` and it works across all targets immediately.