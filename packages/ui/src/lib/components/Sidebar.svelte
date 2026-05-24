<svelte:options runes={true} />
<script lang="ts">
  /**
   * Sidebar — adaptive nav rail (Phase 3.4).
   *
   * Density is driven by the AppShell container query via CSS custom property
   * --rail-density (compact → icons-only → expanded). No density prop needed.
   *
   * Slots:
   *   search    — pinned sticky SidebarSearch (role="search" lives here)
   *   default   — scrollable nav sections (SidebarSection children)
   *   footer    — AccountMenuButton / user chip
   *
   * Usage:
   *   <Sidebar>
   *     {#snippet search()}<SidebarSearch />{/snippet}
   *     <SidebarSection eyebrow="Recent">...</SidebarSection>
   *     {#snippet footer()}<AccountMenuButton />{/snippet}
   *   </Sidebar>
   */
  import type { Snippet } from 'svelte';

  let {
    search,
    children,
    footer,
    class: cls = '',
  }: {
    search?:   Snippet;
    children?: Snippet;
    footer?:   Snippet;
    class?:    string;
  } = $props();
</script>

<div class="sidebar{cls ? ` ${cls}` : ''}">

  <!-- Sticky search row (role="search" is inside SidebarSearch) -->
  {#if search}
    <div class="sidebar-search">
      {@render search()}
    </div>
  {/if}

  <!-- Scrollable sections -->
  <div class="sidebar-scroll">
    {#if children}
      {@render children()}
    {/if}
  </div>

  <!-- Footer: account button -->
  {#if footer}
    <div class="sidebar-footer">
      {@render footer()}
    </div>
  {/if}

</div>

<style>
  .sidebar {
    display:        flex;
    flex-direction: column;
    height:         100%;
    overflow:       hidden;
    background:     var(--color-bg-raised);
  }

  /* ── Search (sticky top) ─────────────────────────────────────────────────── */
  .sidebar-search {
    flex-shrink:  0;
    padding:      var(--space-2);
    border-bottom: 1px solid var(--color-border);
    backdrop-filter: blur(8px);
    background:   var(--color-bg-raised);
    position:     sticky;
    top:          0;
    z-index:      1;
  }

  /* ── Scroll area ─────────────────────────────────────────────────────────── */
  .sidebar-scroll {
    flex:         1;
    overflow-y:   auto;
    overflow-x:   hidden;
    overscroll-behavior: contain;
    padding:      var(--space-1) 0;
  }

  /* ── Footer ──────────────────────────────────────────────────────────────── */
  .sidebar-footer {
    flex-shrink:  0;
    border-top:   1px solid var(--color-border);
    padding:      var(--space-2);
    padding-bottom: calc(var(--space-2) + var(--safe-bottom, 0px));

    /* [hierarchy] Page-load cascade — user chip enters at 360ms */
    animation: cascade-in var(--duration-stagger, 240ms) var(--ease-emphasized-decelerate, ease-out) 360ms both;
  }
</style>
