<svelte:options runes={true} />
<script lang="ts">
  /**
   * SidebarSection — labeled section within the Sidebar (Phase 3.4).
   *
   * Each section has an optional eyebrow label (RECENT, CAPABILITIES, …)
   * and collapses to icons-only in medium density mode.
   *
   * Usage:
   *   <SidebarSection eyebrow="Recent">
   *     <SidebarItem href="/chat/1" icon={MessageSquare}>Project Alpha</SidebarItem>
   *   </SidebarSection>
   */
  import type { Snippet } from 'svelte';

  let {
    eyebrow,
    children,
    class: cls = '',
  }: {
    eyebrow?:  string;
    children?: Snippet;
    class?:    string;
  } = $props();
</script>

<section class="sidebar-section{cls ? ` ${cls}` : ''}">
  {#if eyebrow}
    <header class="section-eyebrow" aria-hidden="true">{eyebrow}</header>
  {/if}
  {#if children}
    <ul class="section-list" role="list">
      {@render children()}
    </ul>
  {/if}
</section>

<style>
  .sidebar-section {
    padding: var(--space-1) 0;
  }
  .sidebar-section + .sidebar-section {
    border-top: 1px solid var(--color-border);
    padding-top: var(--space-2);
    margin-top:  var(--space-1);
  }

  /* Eyebrow */
  .section-eyebrow {
    font-size:      var(--font-size-label);   /* 11px */
    font-weight:    580;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color:          var(--color-fg-subtle);
    padding:        var(--space-2) var(--space-4) var(--space-1);
    line-height:    1;
    white-space:    nowrap;
    overflow:       hidden;
    /* In icon-only mode (medium breakpoint) the eyebrow hides */
    transition:     opacity var(--duration-fast) var(--ease-standard);
  }

  /* Hide eyebrow text when sidebar is collapsed (icons only) */
  @container app-shell (max-width: 1023px) {
    .section-eyebrow { opacity: 0; height: 0; padding: 0; }
  }

  .section-list {
    margin:  0;
    padding: 0;
    list-style: none;
  }
</style>
