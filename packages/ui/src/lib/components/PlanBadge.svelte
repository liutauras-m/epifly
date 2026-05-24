<svelte:options runes={true} />
<script lang="ts">
  import { Layers, Zap, Users, Building2 } from '@lucide/svelte';

  let {
    tier = 'free',
    status = 'active',
  }: {
    tier?: string;
    status?: string;
  } = $props();

  const tierKey = $derived(tier.toLowerCase());
  const isDegraded = $derived(status === 'past_due' || status === 'canceled');
</script>

<span
  class="plan-badge badge-{tierKey}"
  class:degraded={isDegraded}
  title="Plan: {tier} ({status})"
>
  <span class="badge-icon" aria-hidden="true">
    {#if tierKey === 'pro'}
      <Zap size={11} strokeWidth={1.75} />
    {:else if tierKey === 'team'}
      <Users size={11} strokeWidth={1.75} />
    {:else if tierKey === 'enterprise'}
      <Building2 size={11} strokeWidth={1.75} />
    {:else}
      <Layers size={11} strokeWidth={1.75} />
    {/if}
  </span>
  {tier.toUpperCase()}
</span>

<style>
  .plan-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.15rem 0.55rem;
    border-radius: var(--radius-full);
    font-family: var(--font-family-mono);
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    white-space: nowrap;
    line-height: 1.6;
  }

  .badge-icon {
    display: inline-flex;
    align-items: center;
  }

  /* Free — neutral */
  .badge-free {
    background: var(--color-bg-raised);
    color:      var(--color-fg-subtle);
    border:     1px solid var(--color-border);
  }

  /* Pro — accent */
  .badge-pro {
    background: var(--color-accent-soft);
    color:      var(--color-accent);
    border:     1px solid var(--color-accent-border, var(--color-border));
  }

  /* Team — info */
  .badge-team {
    background: var(--color-info-soft, var(--color-accent-soft));
    color:      var(--color-info, var(--color-accent));
    border:     1px solid var(--color-info-border, var(--color-border));
  }

  /* Enterprise — accent with stronger weight */
  .badge-enterprise {
    background:  var(--color-accent-soft);
    color:       var(--color-accent);
    border:      1px solid var(--color-accent-border, var(--color-border));
    font-weight: 700;
  }

  .degraded {
    opacity: 0.55;
    filter: grayscale(0.4);
  }
</style>
