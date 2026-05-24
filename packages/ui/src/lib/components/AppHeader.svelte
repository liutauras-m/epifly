<svelte:options runes={true} />
<script lang="ts">
  /**
   * AppHeader — adaptive app header (Phase 3.3).
   *
   * The canonical topbar — one component for compact / medium / expanded.
   * Layout mode is driven by the AppShell container query (no prop needed).
   *
   * Slots:
   *   leading  — hamburger (compact) / back / nothing (expanded)
   *   title    — workspace name + breadcrumb row
   *   trailing — new-chat icon, theme toggle, profile avatar
   *
   * Platform adaptations:
   *   macOS Tauri : data-tauri-drag-region, traffic-light inset
   *   iOS         : env(safe-area-inset-top) padding
   *
   * Migration shim: AppTopBar.svelte re-exports this with @deprecated tag.
   * Deleted at Phase 4 close via ui:contracts gate.
   */
  import type { Snippet } from 'svelte';

  let {
    leading,
    title,
    trailing,
    class: cls = '',
  }: {
    /** Leading slot — hamburger on compact, back button or nothing on expanded */
    leading?:  Snippet;
    /** Title slot — workspace name, breadcrumb */
    title?:    Snippet;
    /** Trailing slot — actions, theme toggle, profile */
    trailing?: Snippet;
    class?:    string;
  } = $props();
</script>

<!--
  data-tauri-drag-region enables macOS window dragging from the header.
  Traffic-light inset is handled by CSS: padding-left: env(titlebar-area-inset-left).
-->
<header
  class="app-header{cls ? ` ${cls}` : ''}"
  data-tauri-drag-region
>
  <!--
    HTML spec §4.3.3: <header> outside <article>/<aside>/<main>/<nav>/<section>
    carries implicit role="banner" per the HTML AAM. Explicit role is redundant
    and triggers svelte/a11y-no-redundant-roles. The landmark is correctly exposed
    to assistive technology without the attribute.
  -->
  <!-- Safe-area top fill (iOS notch / Dynamic Island / Android status bar) -->
  <div class="header-safe" aria-hidden="true"></div>

  <div class="header-inner">
    {#if leading}
      <div class="header-leading">
        {@render leading()}
      </div>
    {/if}

    <div class="header-title">
      {#if title}
        {@render title()}
      {/if}
    </div>

    {#if trailing}
      <div class="header-trailing">
        {@render trailing()}
      </div>
    {/if}
  </div>
</header>

<style>
  /* ── Header ──────────────────────────────────────────────────────────────── */
  .app-header {
    background:     var(--color-bg);
    border-bottom:  1px solid var(--color-border);
    flex-shrink:    0;
    z-index:        var(--z-topbar, 100);

    /* macOS Tauri traffic-light inset */
    padding-left:   env(titlebar-area-inset-left, 0px);
  }

  /* Safe-area top (iOS notch / Dynamic Island). Height = inset, bg matches header. */
  .header-safe {
    height:     var(--safe-top, 0px);
    background: inherit;
  }

  /* ── Inner row ───────────────────────────────────────────────────────────── */
  .header-inner {
    display:        flex;
    align-items:    center;
    height:         var(--topbar-height);
    padding:        0 var(--space-2);
    gap:            var(--space-1);
  }

  /* ── Slots ───────────────────────────────────────────────────────────────── */
  .header-leading {
    display:     flex;
    align-items: center;
    flex-shrink: 0;
    min-width:   var(--hit, 44px);
  }

  .header-title {
    flex:           1;
    min-width:      0;
    overflow:       hidden;
    display:        flex;
    align-items:    center;

    /* Compact: centered */
    justify-content: center;

    font-family:    var(--font-family-sans);
    font-size:      var(--font-size-h2);     /* 20px */
    font-weight:    580;
    letter-spacing: -0.018em;
    color:          var(--color-fg);
    white-space:    nowrap;
    text-overflow:  ellipsis;
  }

  .header-trailing {
    display:        flex;
    align-items:    center;
    gap:            var(--space-1);
    flex-shrink:    0;
    min-width:      var(--hit, 44px);
    justify-content: flex-end;
  }

  /* ── Medium / expanded: left-align title, smaller height ───────────────── */
  @container app-shell (min-width: 768px) {
    .header-inner {
      height:  var(--topbar-height-compact);
      padding: 0 var(--space-3);
    }
    .header-title {
      justify-content: flex-start;
      font-size:       var(--font-size-body);   /* 15px */
      font-weight:     500;
    }
  }

  /* ── Expanded: even less chrome ─────────────────────────────────────────── */
  @container app-shell (min-width: 1024px) {
    .header-inner {
      height:  var(--topbar-height-expanded);
      padding: 0 var(--space-4);
      gap:     var(--space-2);
    }
  }
</style>
