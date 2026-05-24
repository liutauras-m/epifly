<svelte:options runes={true} />
<script lang="ts">
  /**
   * ShellScreen — authenticated application shell (Phase 3.1)
   *
   * Composes AppShell + AppHeader + Sidebar + screen-routing into a
   * single reusable feature component consumed by:
   *   - apps/browser-shell/src/routes/+page.svelte   (Tauri mobile/desktop)
   *   - apps/web/src/routes/+page.svelte             (SvelteKit web)
   *
   * Props
   *   sdk         — ConusSdk instance (data loading)
   *   chatStream  — from createChatStream(sdk)
   *   userName    — display name shown in navigation header
   *   userPlan    — plan tier label
   *   sigil       — optional logo image URL for the greeting empty state
   *   appTitle    — page / tab title (default "ConusAI")
   *
   * Callbacks
   *   onLogout      — called when user clicks sign-out
   */
  import type { ConusSdk } from '@conusai/sdk';
  import type { WorkspaceNode } from '@conusai/types';
  import type { Snippet } from 'svelte';

  import AppShell          from '../components/AppShell.svelte';
  import AppHeader         from '../components/AppHeader.svelte';
  import Sidebar           from '../components/Sidebar.svelte';
  import SidebarSection    from '../components/SidebarSection.svelte';
  import SidebarItem       from '../components/SidebarItem.svelte';
  import WorkspaceTree from './WorkspaceTree.svelte';
  import DrawerRecentChats from './DrawerRecentChats.svelte';
  import ThemeSwitcher     from '../components/ThemeSwitcher.svelte';

  import ChatScreen         from './screens/ChatScreen.svelte';
  import CapabilitiesScreen from './screens/CapabilitiesScreen.svelte';
  import ArtifactsScreen    from './screens/ArtifactsScreen.svelte';
  import { buildInvocationPrompt } from './screens/buildInvocationPrompt.js';
  import type { CapEntry }         from './CapabilityBrowser.svelte';

  import { registerKeyboardShortcuts } from '../utils/keyboard.js';

  // Nav icons for capabilities + artifacts sidebar items
  const svgCapabilities = `<rect x="2" y="3" width="6" height="6" rx="1"/><rect x="16" y="3" width="6" height="6" rx="1"/><rect x="2" y="15" width="6" height="6" rx="1"/><rect x="16" y="15" width="6" height="6" rx="1"/><line x1="8" y1="6" x2="16" y2="6"/><line x1="8" y1="18" x2="16" y2="18"/><line x1="5" y1="9" x2="5" y2="15"/><line x1="19" y1="9" x2="19" y2="15"/>`;
  const svgArtifacts    = `<path d="M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9z"/><polyline points="13 2 13 9 20 9"/>`;

  import { startViewTransition } from '../motion/index.js';
  import {
    screenStore,
    drawerStore,
    breadcrumbsStore,
    recentsStore,
  } from '../stores/index.js';

  // ── SVG icon paths (inline, topbar-only) ────────────────────────────────────
  const iconHamburger = `<line x1="3" y1="6" x2="21" y2="6"/><line x1="3" y1="12" x2="21" y2="12"/><line x1="3" y1="18" x2="21" y2="18"/>`;
  const iconPlus      = `<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>`;
  const iconBack      = `<polyline points="15 18 9 12 15 6"/>`;
  const iconLogOut    = `<path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/>`;

  let {
    sdk,
    chatStream,
    userName,
    userPlan = '',
    sigil,
    appTitle = 'ConusAI',
    onLogout,
    selectedNode = $bindable<WorkspaceNode | null>(null),
  }: {
    sdk:          ConusSdk;
    chatStream:   any;
    userName:   string;
    userPlan?:  string;
    sigil?:     string;
    appTitle?:  string;
    onLogout?:  () => void;
    /** Bindable: lets the parent seed the selected workspace node. */
    selectedNode?: WorkspaceNode | null;
  } = $props();

  // ── State ─────────────────────────────────────────────────────────────────
  let workspaceNodes    = $state<WorkspaceNode[]>([]);
  let lastInvalidKey    = $state<string | null>(null);
  let composerRef       = $state<{ focus(): void } | undefined>();
  /** Ref to the hamburger button — focus restored here when drawer closes. */
  let hamburgerEl       = $state<HTMLButtonElement | undefined>();

  async function refreshWorkspace() {
    const result = await sdk.workspaces.tree();
    if (!result.error && Array.isArray(result.data)) {
      workspaceNodes = result.data;
    }
  }

  // Load tree on mount
  $effect(() => {
    refreshWorkspace();
  });

  // ── Initials calculation helper ──────────────────────────────────────────
  function getInitials(name: string): string {
    if (!name) return '?';
    const parts = name.trim().split(/\s+/);
    if (parts.length >= 2) {
      return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
    }
    return name.slice(0, 2).toUpperCase();
  }

  // ── Derived screen title ──────────────────────────────────────────────────
  const screenTitle = $derived(
    screenStore.active === 'capabilities' ? 'Capabilities' :
    screenStore.active === 'artifacts'    ? 'Artifacts' :
    breadcrumbsStore.node?.name ?? appTitle
  );

  // ── Keyboard shortcuts ────────────────────────────────────────────────────
  $effect(() => {
    return registerKeyboardShortcuts({
      onFocusComposer: () => {
        if (composerRef) {
          composerRef.focus();
        } else if (typeof document !== 'undefined') {
          (document.querySelector('textarea.composer-input') as HTMLTextAreaElement | null)?.focus();
        }
      },
      onEscape: () => {
        if (drawerStore.open) {
          drawerStore.close();
          hamburgerEl?.focus();   // restore focus to the trigger element (WCAG 2.4.3)
          return;
        }
        if (screenStore.active !== 'chat') screenStore.setActive('chat');
      },
      onCommandPalette: () => { /* future */ },
    });
  });

  // ── Navigation callbacks ──────────────────────────────────────────────────
  function onSelectNode(node: WorkspaceNode) {
    selectedNode = node;
    breadcrumbsStore.set(node);
    recentsStore.add(node.id);
    if (node.kind === 'conversation' && (node as any).metadata?.thread_id) {
      chatStream.loadThread?.((node as any).metadata.thread_id);
    } else if (node.kind === 'conversation') {
      chatStream.newSession();
    }
    startViewTransition(() => screenStore.setActive('chat'));
    drawerStore.close();
  }

  function handleNewChat() {
    chatStream.newSession();
    selectedNode = null;
    breadcrumbsStore.clear();
    startViewTransition(() => screenStore.setActive('chat'));
    drawerStore.close();
  }

  function handleCapabilitiesNav() {
    startViewTransition(() => screenStore.setActive('capabilities'));
    drawerStore.close();
  }

  function handleArtifactsNav() {
    startViewTransition(() => screenStore.setActive('artifacts'));
    drawerStore.close();
  }

  function handleInvoke(cap: CapEntry) {
    screenStore.setActive('chat');
    const prompt = buildInvocationPrompt(cap);
    chatStream.send(prompt, {
      workspaceNodeId: selectedNode?.id,
      forcedCapability: cap.name,
    });
  }

  // ── Workspace revalidation on resource_invalidated ────────────────────────
  $effect(() => {
    const inv = chatStream.lastInvalidation;
    if (inv && inv.resource === 'workspace') {
      const key = JSON.stringify(inv);
      if (key !== lastInvalidKey) {
        lastInvalidKey = key;
        refreshWorkspace();
      }
    }
  });
</script>

<svelte:head><title>{screenTitle} · {appTitle}</title></svelte:head>

<svelte:window onkeydown={(e) => {
  const mod = e.metaKey || e.ctrlKey;
  if (mod && e.key === 'n') {
    e.preventDefault();
    handleNewChat();
  }
}} />

<!-- Sidebar footer: user chip → logout.
     Defined at top-level snippet scope so it can be passed into
     <Sidebar footer={...}> without Svelte treating it as an AppShell prop. -->
{#snippet footerSnippet()}
  <a
    href="/logout"
    class="user-chip"
    aria-label="Logout — {userName}"
    title="Logout"
    onclick={(e) => {
      e.preventDefault();
      onLogout?.();
    }}
  >
    <div class="avatar" aria-hidden="true">{getInitials(userName)}</div>
    <div class="user-meta">
      <span class="user-name">{userName}</span>
      {#if userPlan}<span class="user-plan">{userPlan}</span>{/if}
    </div>
  </a>
{/snippet}

<!-- Shared sidebar nav content -->
{#snippet navContent()}
  <WorkspaceTree
    {sdk}
    bind:nodes={workspaceNodes}
    selectedNodeId={selectedNode?.id}
    {onSelectNode}
  />

  <DrawerRecentChats
    {sdk}
    tenantId={null}
    {chatStream}
    onSelect={(thread) => {
      chatStream.loadThread?.(thread.id);
      breadcrumbsStore.clear();
      selectedNode = null;
      screenStore.setActive('chat');
      drawerStore.close();
    }}
  />

  <SidebarSection>
    <ul role="list" class="nav-list">
      <SidebarItem
        active={screenStore.active === 'capabilities'}
        onclick={handleCapabilitiesNav}
        class="shell-nav-item"
      >
        <span class="nav-item-inner">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="20" height="20" aria-hidden="true">{@html svgCapabilities}</svg>
          <span class="nav-item-label">Capabilities</span>
        </span>
      </SidebarItem>
      <SidebarItem
        active={screenStore.active === 'artifacts'}
        onclick={handleArtifactsNav}
        class="shell-nav-item"
      >
        <span class="nav-item-inner">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="20" height="20" aria-hidden="true">{@html svgArtifacts}</svg>
          <span class="nav-item-label">Artifacts</span>
        </span>
      </SidebarItem>
    </ul>
  </SidebarSection>
{/snippet}

<AppShell
  sidebarRole="navigation"
  sidebarLabel="Workshop navigation"
  bind:open={drawerStore.open}
  onclose={() => hamburgerEl?.focus()}
>
  <!-- ── Topbar ──────────────────────────────────────────────────────────── -->
  {#snippet topbar()}
    <AppHeader>
      {#snippet leading()}
        <button
          bind:this={hamburgerEl}
          class="icon-btn shell-hamburger"
          aria-label="Toggle nav"
          aria-expanded={drawerStore.open}
          onclick={() => drawerStore.toggle()}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="22" height="22" aria-hidden="true">
            {@html iconHamburger}
          </svg>
        </button>
        {#if screenStore.canGoBack}
          <button class="icon-btn" aria-label="Go back" onclick={() => screenStore.pop()}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="22" height="22" aria-hidden="true">
              {@html iconBack}
            </svg>
          </button>
        {/if}
      {/snippet}

      {#snippet title()}
        {screenTitle}
      {/snippet}

      {#snippet trailing()}
        {#if screenStore.active === 'chat'}
          <button class="icon-btn" aria-label="New conversation" onclick={handleNewChat}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="22" height="22" aria-hidden="true">
              {@html iconPlus}
            </svg>
          </button>
        {/if}
        <ThemeSwitcher />
        {#if onLogout}
          <a
            href="/logout"
            class="icon-btn"
            aria-label="Sign out"
            title="Sign out"
            onclick={(e) => {
              e.preventDefault();
              onLogout();
            }}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="22" height="22" aria-hidden="true">
              {@html iconLogOut}
            </svg>
          </a>
        {/if}
      {/snippet}
    </AppHeader>
  {/snippet}

  <!-- ── Persistent sidebar (medium ≥ 768px) ──────────────────────────────── -->
  {#snippet sidebar()}
    <Sidebar footer={onLogout ? footerSnippet : undefined}>
      {@render navContent()}
    </Sidebar>
  {/snippet}

  <!-- ── Main content ────────────────────────────────────────────────────── -->
  {#snippet main()}
    <div class="main-transition-root">
      {#if screenStore.active === 'chat'}
        <ChatScreen
          {sdk}
          {chatStream}
          selectedNode={selectedNode}
          onSelectNode={(n) => {
            if (n) { selectedNode = n; breadcrumbsStore.set(n); }
          }}
          userName={userName}
          {sigil}
          bind:composerRef
        />
      {:else if screenStore.active === 'capabilities'}
        <CapabilitiesScreen {sdk} onInvoke={handleInvoke} />
      {:else if screenStore.active === 'artifacts'}
        <ArtifactsScreen {sdk} />
      {/if}
    </div>
  {/snippet}
</AppShell>

<style>
  /* ── Icon button (topbar actions) ─────────────────────────────────────────── */
  .icon-btn {
    display:         flex;
    align-items:     center;
    justify-content: center;
    width:           var(--hit, 44px);
    height:          var(--hit, 44px);
    border:          none;
    background:      transparent;
    color:           var(--color-fg-muted);
    cursor:          pointer;
    border-radius:   var(--radius-sm);
    padding:         0;
    text-decoration: none;
    transition:
      background var(--duration-fast) var(--ease-standard), /* [feedback] */
      color      var(--duration-fast) var(--ease-standard); /* [feedback] */
  }
  .icon-btn:hover        { background: var(--color-bg-hover); color: var(--color-fg); }
  .icon-btn:focus-visible { outline: var(--focus-ring); outline-offset: var(--focus-ring-offset); }

  /* Hide hamburger on medium+ (sidebar is persistent) */
  .shell-hamburger { display: flex; }
  @container app-shell (min-width: 768px) {
    .shell-hamburger { display: none; }
  }

  /* ── Nav list ─────────────────────────────────────────────────────────────── */
  .nav-list { list-style: none; margin: 0; padding: 0; }

  /* ── User chip (sidebar footer) ───────────────────────────────────────────── */
  .user-chip {
    display:        flex;
    align-items:    center;
    gap:            var(--space-2);
    padding:        var(--space-2) var(--space-3);
    border:         none;
    background:     transparent;
    color:          inherit;
    cursor:         pointer;
    text-align:     left;
    border-radius:  var(--radius-sm);
    min-height:     var(--hit, 44px);
    width:          100%;
    text-decoration: none;
    transition:     background var(--duration-fast) var(--ease-standard); /* [feedback] */
  }
  .user-chip:hover        { background: var(--color-bg-hover); }
  .user-chip:focus-visible { outline: var(--focus-ring); outline-offset: var(--focus-ring-offset); }

  .avatar {
    width:           28px;
    height:          28px;
    border-radius:   50%;
    background:      var(--color-accent-soft);
    border:          1px solid var(--color-border);
    display:         flex;
    align-items:     center;
    justify-content: center;
    font-size:       var(--font-size-label);
    font-weight:     600;
    color:           var(--color-accent);
    flex-shrink:     0;
    user-select:     none;
  }

  .user-meta {
    display:        flex;
    flex-direction: column;
    flex:           1;
    min-width:      0;
  }
  .user-name {
    font-size:     var(--font-size-meta);
    color:         var(--color-fg);
    overflow:      hidden;
    text-overflow: ellipsis;
    white-space:   nowrap;
  }
  .user-plan {
    font-size:      var(--font-size-label);
    color:          var(--color-fg-subtle);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  /* Hide user meta in icon-only rail */
  @container app-shell (max-width: 1023px) {
    .user-meta  { display: none; }
    .avatar     { margin: auto; }
  }

  /* ── Main-content view-transition wrapper ─────────────────────────────────── */
  .main-transition-root {
    display:              contents;
    view-transition-name: main-content;
    contain:              layout;
  }

  /* ── View transition cross-fade [continuity] ──────────────────────────────── */
  :global(::view-transition-old(main-content)) {
    animation: vt-fade-out var(--duration-normal, 200ms) var(--ease-standard, ease) both;
  }
  :global(::view-transition-new(main-content)) {
    animation: vt-fade-in var(--duration-normal, 200ms) var(--ease-standard, ease) both;
  }

  @keyframes vt-fade-out {
    from { opacity: 1; transform: translateY(0); }
    to   { opacity: 0; transform: translateY(-6px); }
  }
  @keyframes vt-fade-in {
    from { opacity: 0; transform: translateY(8px); }
    to   { opacity: 1; transform: translateY(0); }
  }

  /* Reduced motion: opacity cross-fade only */
  @media (prefers-reduced-motion: reduce) {
    :global(::view-transition-old(main-content)) {
      animation-duration: 80ms !important;
      animation-name: vt-opacity-out !important;
    }
    :global(::view-transition-new(main-content)) {
      animation-duration: 80ms !important;
      animation-name: vt-opacity-in !important;
    }
    @keyframes vt-opacity-out { from { opacity: 1; } to { opacity: 0; } }
    @keyframes vt-opacity-in  { from { opacity: 0; } to { opacity: 1; } }
  }

  @media (prefers-reduced-motion: reduce) {
    .icon-btn, .user-chip { transition: none; }
  }
</style>
