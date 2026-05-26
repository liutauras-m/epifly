<svelte:options runes={true} />
<script lang="ts">
  /**
   * Breadcrumbs — semantic breadcrumb navigation primitive (Phase 3.3).
   *
   * Renders an accessible `<nav aria-label="Breadcrumb">` with a list of
   * items. The last item is the current page (no link, aria-current="page").
   * All preceding items are links.
   *
   * Usage:
   *   <Breadcrumbs items={[
   *     { label: 'Account', href: '/account' },
   *     { label: 'Billing' },
   *   ]} />
   */

  export interface BreadcrumbItem {
    label: string;
    href?: string;
  }

  let {
    items,
    class: cls = '',
  }: {
    items: BreadcrumbItem[];
    class?: string;
  } = $props();
</script>

<nav class="breadcrumbs {cls}" aria-label="Breadcrumb">
  <ol class="crumb-list">
    {#each items as item, i}
      {@const isLast = i === items.length - 1}
      <li class="crumb-item">
        {#if isLast}
          <span class="crumb-current" aria-current="page">{item.label}</span>
        {:else}
          <a class="crumb-link" href={item.href ?? '#'}>{item.label}</a>
          <span class="crumb-sep" aria-hidden="true">›</span>
        {/if}
      </li>
    {/each}
  </ol>
</nav>

<style>
  .crumb-list {
    display:     flex;
    flex-wrap:   wrap;
    align-items: center;
    gap:         var(--space-2);
    list-style:  none;
    margin:      0;
    padding:     0;
    font-family: var(--font-family-mono);
    font-size:   var(--font-size-meta);
    color:       var(--color-fg-subtle);
  }

  .crumb-item {
    display:     flex;
    align-items: center;
    gap:         var(--space-2);
  }

  .crumb-link {
    color:           var(--color-accent);
    text-decoration: none;
    font-weight:     500;
  }
  .crumb-link:hover {
    text-decoration: underline;
  }
  .crumb-link:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
    border-radius:  var(--radius-xs);
  }

  .crumb-sep {
    /* Already in gap — just the › divider */
    user-select: none;
  }
</style>
