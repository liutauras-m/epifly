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
   *   sidebarRole='navigation' (default) → aria-label="Workspace"
   *   sidebarRole='complementary'        → right-panel supplemental content
   *   No sidebar slot → no landmark emitted
   *
   * WCAG 2.2 verified: banner + main appear exactly once per page.
   * form[aria-label="Message composer"] is the ONLY composer landmark.
   */
  import type { Snippet } from 'svelte';
  import '../tokens.css';

  let {
    topbar,
    sidebar,
    main:     mainContent,
    composer,
    overlay,
    sidebarRole  = 'navigation' as 'navigation' | 'complementary',
    composerLabel = 'Message composer',
    class: cls = '',
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
    /** aria-label for the composer <form>. Default 'Message composer'. */
    composerLabel?: string;
    class?:        string;
  } = $props();
</script>

<!--
  .app-shell is the container-query root.
  All breakpoints are expressed as `@container app-shell (…)` — no viewport media.
-->
<div class="app-shell{cls ? ` ${cls}` : ''}">

  <!-- Topbar ──────────────────────────────────────────────── -->
  {#if topbar}
    <header class="shell-topbar" role="banner">
      {@render topbar()}
    </header>
  {/if}

  <!-- Body row (sidebar + main) ───────────────────────────── -->
  <div class="shell-body">

    <!-- Sidebar / Rail ──────────────────────────────────────── -->
    {#if sidebar}
      {#if sidebarRole === 'navigation'}
        <nav class="shell-sidebar" aria-label="Workspace">
          {@render sidebar()}
        </nav>
      {:else}
        <aside class="shell-sidebar" aria-label="Sidebar">
          {@render sidebar()}
        </aside>
      {/if}
    {/if}

    <!-- Main content ────────────────────────────────────────── -->
    <main class="shell-main" tabindex="-1" id="main-content">
      {#if mainContent}
        {@render mainContent()}
      {/if}
    </main>

  </div>

  <!-- Composer ────────────────────────────────────────────── -->
  {#if composer}
    <form class="shell-composer" aria-label={composerLabel} onsubmit={(e) => e.preventDefault()}>
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
    flex-shrink:    0;
    overflow-y:     auto;
    overflow-x:     hidden;
    background:     var(--color-bg-raised);
    border-right:   1px solid var(--color-border);

    /* Compact: hidden by default — Drawer opens it */
    display:        none;
    width:          0;
    transition:     width var(--duration-normal) var(--ease-standard);
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
