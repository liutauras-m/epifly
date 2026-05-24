<svelte:options runes={true} />
<script lang="ts">
  /**
   * AppShell — single adaptive layout container (Phase 3.1).
   *
   * Replaces the old two-column flex stub. All consumer apps render their
   * layout via AppShell and supply slot content — never local CSS layout.
   *
   * Layout regions (named Svelte 5 snippets):
   *   topbar   → <header role="banner">  — brand, hamburger, actions
   *   sidebar  → <nav|aside>             — rail / workspace nav
   *   main     → <main>                  — primary scrollable content
   *   composer → <form aria-label="…">   — message input (chat screens only)
   *   overlay  → positioned above main   — toasts, sheets, drawers
   *
   * Three breakpoints via named @container queries (never viewport media —
   * works inside Tauri windows of any arbitrary size):
   *   compact   < 768px  : sidebar hidden, topbar with hamburger, composer fixed bottom
   *   medium   768-1023px: sidebar collapsed 64px (icons only)
   *   expanded ≥ 1024px  : sidebar full 260px, persistent
   *
   * A11y:
   *   sidebarRole='navigation' (default) → aria-label="Workshop navigation"
   *   sidebarRole='complementary'        → right-panel supplemental content
   *   No sidebar slot → no landmark emitted
   *
   * WCAG 2.2 verified: banner + main appear exactly once per page.
   * form[aria-label="Message composer"] is the ONLY composer landmark.
   */
  import type { Snippet } from 'svelte';
  import '../tokens.css';
  import { t } from '../utils/i18n.js';

  let {
    topbar,
    sidebar,
    main:     mainContent,
    composer,
    overlay,
    sidebarRole  = 'navigation' as 'navigation' | 'complementary',
    sidebarLabel = 'Workshop navigation',
    composerLabel = 'Message composer',
    class: cls = '',
    open = $bindable(false),
    onclose,
  }: {
    /** Topbar content — rendered inside <header role="banner"> */
    topbar?:       Snippet;
    /** Sidebar / rail content — rendered inside <nav> or <aside> per sidebarRole */
    sidebar?:      Snippet;
    /** Primary page content — rendered inside <main> */
    main?:         Snippet;
    /** Composer content — rendered inside <form aria-label> on chat screens only */
    composer?:     Snippet;
    /** Overlay layer — toasts, drawers, sheets */
    overlay?:      Snippet;
    /** ARIA role for the sidebar landmark. Default 'navigation'. */
    sidebarRole?:  'navigation' | 'complementary';
    /** aria-label for the sidebar. Default 'Workshop navigation'. */
    sidebarLabel?: string;
    /** aria-label for the composer <form>. Default 'Message composer'. */
    composerLabel?: string;
    class?:        string;
    /** Drawer open state for compact/mobile layouts */
    open?:         boolean;
    /** Called when the backdrop closes the drawer (focus-restoration hook). */
    onclose?:      () => void;
  } = $props();

  // ── Sidebar ref for focus management ─────────────────────────────────────
  let sidebarEl = $state<HTMLElement | undefined>();

  // When the drawer opens: move focus to first interactive sidebar item.
  $effect(() => {
    if (open) {
      const id = requestAnimationFrame(() => {
        const first = sidebarEl?.querySelector<HTMLElement>(
          'a[href]:not([tabindex="-1"]), button:not([disabled]):not([tabindex="-1"]), [tabindex="0"]',
        );
        first?.focus();
      });
      return () => cancelAnimationFrame(id);
    }
  });

  // Auto-close the mobile drawer when the viewport widens to the persistent
  // sidebar breakpoint — prevents the drawer from being "stuck open" after resize.
  $effect(() => {
    if (typeof window === 'undefined') return;
    const mq = window.matchMedia('(min-width: 768px)');
    function onChange(e: MediaQueryListEvent) {
      if (e.matches && open) open = false;
    }
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  });
</script>

<!--
  .app-shell is the container-query root.
  All breakpoints are expressed as `@container app-shell (…)` — no viewport media.
  Skip links are injected here (Phase 7) — visually hidden until focused.
-->
<div class="app-shell{cls ? ` ${cls}` : ''}">

  <!-- Skip navigation links (Phase 7 a11y) ───────────────── -->
  <nav class="skip-links" aria-label="Skip navigation">
    <a class="skip-link" href="#main-content">{t('nav.skip_to_main')}</a>
    {#if composer}
      <a class="skip-link" href="#composer-input">{t('nav.skip_to_composer')}</a>
    {/if}
  </nav>

  <!-- Topbar ──────────────────────────────────────────────── -->
  {#if topbar}
    <header class="shell-topbar">
      {@render topbar()}
    </header>
  {/if}

  <!-- Body row (sidebar + main) ───────────────────────────── -->
  <div class="shell-body">

    <!-- Sidebar / Rail ──────────────────────────────────────── -->
    {#if sidebar}
      {#if sidebarRole === 'navigation'}
        <nav class="shell-sidebar" class:open aria-label={sidebarLabel} bind:this={sidebarEl}>
          {@render sidebar()}
        </nav>
      {:else}
        <aside class="shell-sidebar" class:open aria-label={sidebarLabel} bind:this={sidebarEl}>
          {@render sidebar()}
        </aside>
      {/if}
      <!-- Backdrop for compact mobile drawer.
           tabindex=-1 when closed keeps it out of the tab order;
           pointer-events:none in CSS ensures it is inert to mouse too. -->
      <button
        type="button"
        class="shell-sidebar-backdrop"
        class:open
        tabindex={open ? 0 : -1}
        onclick={() => { open = false; onclose?.(); }}
        aria-label="Close navigation"
      ></button>
    {/if}

    <!-- Main content — inert while mobile drawer is open so keyboard
         navigation cannot escape into the obscured content area. -->
    <main class="shell-main" tabindex="-1" id="main-content" inert={open || undefined}>
      {#if mainContent}
        {@render mainContent()}
      {/if}
    </main>

  </div>

  <!-- Composer — also inert while mobile drawer is open. -->
  {#if composer}
    <form
      class="shell-composer composer"
      id="composer-input"
      aria-label={composerLabel}
      onsubmit={(e) => e.preventDefault()}
      inert={open || undefined}
    >
      {@render composer()}
    </form>
  {/if}

  <!-- Overlay layer ───────────────────────────────────────── -->
  {#if overlay}
    <div class="shell-overlay" aria-live="polite">
      {@render overlay()}
    </div>
  {/if}

</div>

<style>
  /* ── Skip links (Phase 7) ───────────────────────────────────────────────── */
  .skip-links {
    position:        absolute;
    z-index:         9999;
    inset-inline-start: 0;
    top:             0;
    display:         flex;
    gap:             var(--space-2);
  }

  .skip-link {
    /* Visually hidden until focused */
    position:    absolute;
    inset-inline-start: var(--space-3);
    top:         var(--space-3);
    transform:   translateY(-200%);
    padding:     var(--space-2) var(--space-4);
    background:  var(--color-accent);
    color:       var(--color-on-accent);
    font-size:   var(--font-size-meta);
    font-weight: 600;
    border-radius: var(--radius-sm);
    text-decoration: none;
    outline:     none;

    transition: transform var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }

  .skip-link:focus {
    transform: translateY(0);
  }

  .skip-links .skip-link:nth-child(2):focus {
    inset-inline-start: calc(var(--space-3) + 180px);
  }

  /* ── Container setup ─────────────────────────────────────────────────────── */
  .app-shell {
    container-type: inline-size;
    container-name: app-shell;

    display:        flex;
    flex-direction: column;
    height:         100dvh;
    overflow:       hidden;

    background:     var(--color-bg);
    color:          var(--color-fg);
    font-family:    var(--font-family-sans);
  }

  /* ── Topbar ──────────────────────────────────────────────────────────────── */
  .shell-topbar {
    flex-shrink:  0;
    z-index:      var(--z-topbar, 100);
    /* topbar components handle their own internal layout & safe-area padding */
  }

  /* ── Body row ────────────────────────────────────────────────────────────── */
  .shell-body {
    flex:         1;
    display:      flex;
    overflow:     hidden;
    min-height:   0;
  }

  /* ── Sidebar ─────────────────────────────────────────────────────────────── */
  .shell-sidebar {
    flex-shrink:      0;
    overflow-y:       auto;
    overflow-x:       hidden;
    background:       var(--color-bg-raised);
    border-inline-end: 1px solid var(--color-border);  /* logical — RTL flips to border-left */
  }

  /* Compact: styled as premium edge-slide drawer */
  @container app-shell (max-width: 767px) {
    .shell-sidebar {
      display:          flex;
      flex-direction:   column;
      position:         fixed;
      top:              0;
      bottom:           0;
      left:             0;
      width:            17.5rem;
      z-index:          var(--z-drawer, 300);
      transform:        translateX(-100%);
      transition:       transform var(--duration-normal, 200ms) var(--ease-standard); /* [continuity] */
      box-shadow:       var(--shadow-lg, 0 10px 25px -5px rgba(0,0,0,0.1), 0 8px 10px -6px rgba(0,0,0,0.1));
    }
    
    .shell-sidebar.open {
      transform:        translateX(0);
    }

    /* RTL edge slide flip */
    :global([dir="rtl"]) .shell-sidebar {
      left:             auto;
      right:            0;
      transform:        translateX(100%);
      border-inline-end: none;
      border-inline-start: 1px solid var(--color-border);
    }

    :global([dir="rtl"]) .shell-sidebar.open {
      transform:        translateX(0);
    }
  }

  .shell-sidebar-backdrop {
    display: none;
  }

  @container app-shell (max-width: 767px) {
    .shell-sidebar-backdrop {
      display:          block;
      position:         fixed;
      inset:            0;
      background:       var(--color-backdrop, rgba(0, 0, 0, 0.4));
      z-index:          calc(var(--z-drawer, 300) - 1);
      border:           none;
      padding:          0;
      cursor:           pointer;
      opacity:          0;
      pointer-events:   none;
      transition:       opacity var(--duration-normal, 200ms) var(--ease-standard); /* [continuity] */
    }

    .shell-sidebar-backdrop.open {
      opacity:          1;
      pointer-events:   auto;
    }
  }

  /* Medium breakpoint: collapsed icon rail */
  @container app-shell (min-width: 768px) {
    .shell-sidebar {
      display: flex;
      flex-direction: column;
      width: var(--sidebar-collapsed, 64px);
    }
  }

  /* Expanded breakpoint: full sidebar */
  @container app-shell (min-width: 1024px) {
    .shell-sidebar {
      width: var(--sidebar, 260px);
    }
  }

  /* ── Main ────────────────────────────────────────────────────────────────── */
  .shell-main {
    flex:           1;
    overflow-y:     auto;
    overflow-x:     hidden;
    min-width:      0;
    outline:        none;  /* tabindex="-1" for skip-link target */
  }

  /* ── Composer ────────────────────────────────────────────────────────────── */
  .shell-composer {
    flex-shrink:   0;
    /* On compact: pinned to bottom above safe-area.
       Composer component handles its own internal padding/shadow. */
    padding-bottom: var(--safe-bottom, 0px);
    border-top:     1px solid var(--color-border);
    background:     var(--color-bg);
  }

  /* ── Overlay ─────────────────────────────────────────────────────────────── */
  .shell-overlay {
    position:  absolute;
    inset:     0;
    pointer-events: none;
    z-index:   var(--z-overlay, 200);
  }
  .shell-overlay > :global(*) {
    pointer-events: auto;
  }
</style>
