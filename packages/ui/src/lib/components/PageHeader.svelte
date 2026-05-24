<svelte:options runes={true} />
<script lang="ts">
  /**
   * PageHeader — canonical page-level heading primitive (Phase 4.4).
   *
   * Renders an optional mono-weight eyebrow label above the main title
   * with an optional subtitle beneath. Used on `/account`, `/account/billing`,
   * `/account/usage`, and any route that follows the section-header pattern.
   *
   * Usage:
   *   <PageHeader eyebrow="ACCOUNT" title="Account" />
   *   <PageHeader eyebrow="BILLING" title="Billing & Plans" subtitle="Manage your subscription." />
   *   <PageHeader title="Usage" {children} />
   *
   * Slots (snippets)
   *   children — optional trailing content (action buttons, etc.)
   */
  import type { Snippet } from 'svelte';

  let {
    eyebrow,
    title,
    subtitle,
    class: cls = '',
    children,
  }: {
    eyebrow?:  string;
    title:     string;
    subtitle?: string;
    class?:    string;
    children?: Snippet;
  } = $props();
</script>

<header class="page-header{cls ? ` ${cls}` : ''}">
  {#if eyebrow}
    <p class="page-eyebrow" aria-hidden="true">{eyebrow}</p>
  {/if}
  <div class="page-header-row">
    <h1 class="page-title">{title}</h1>
    {#if children}
      <div class="page-header-actions">
        {@render children()}
      </div>
    {/if}
  </div>
  {#if subtitle}
    <p class="page-subtitle">{subtitle}</p>
  {/if}
</header>

<style>
  .page-header {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-1);
    margin-bottom:  var(--space-5);
  }

  .page-eyebrow {
    margin:         0;
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);   /* 11px */
    font-weight:    500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color:          var(--color-fg-subtle);
  }

  .page-header-row {
    display:     flex;
    align-items: baseline;
    gap:         var(--space-4);
  }

  .page-title {
    margin:         0;
    font-family:    var(--font-family-sans);
    font-size:      var(--font-size-h1);      /* 28px */
    font-weight:    620;
    letter-spacing: -0.025em;
    color:          var(--color-fg);
    line-height:    1.2;
    flex:           1;
    min-width:      0;
  }

  .page-header-actions {
    display:     flex;
    align-items: center;
    gap:         var(--space-2);
    flex-shrink: 0;
  }

  .page-subtitle {
    margin:      0;
    font-family: var(--font-family-sans);
    font-size:   var(--font-size-meta);       /* 13px */
    color:       var(--color-fg-subtle);
    line-height: 1.5;
  }
</style>
