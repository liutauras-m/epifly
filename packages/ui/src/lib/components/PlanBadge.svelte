<script lang="ts">
  import { Layers, Zap, Users, Building2 } from 'lucide-svelte';

  export let tier: string = 'free';
  export let status: string = 'active';

  const tierKey = () => tier.toLowerCase();
  $: isDegraded = status === 'past_due' || status === 'canceled';
</script>

<span
  class="plan-badge badge-{tierKey()}"
  class:degraded={isDegraded}
  title="Plan: {tier} ({status})"
>
  <span class="badge-icon" aria-hidden="true">
    {#if tierKey() === 'pro'}
      <Zap size={11} strokeWidth={1.75} />
    {:else if tierKey() === 'team'}
      <Users size={11} strokeWidth={1.75} />
    {:else if tierKey() === 'enterprise'}
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
    font-family: var(--font-mono);
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

  /* Free — neutral ink */
  .badge-free {
    background: var(--paper-2);
    color: var(--ink-3);
    border: 1px solid var(--rule);
  }

  /* Pro — ember accent */
  .badge-pro {
    background: var(--ember-soft);
    color: var(--ember);
    border: 1px solid var(--ember-glow);
  }

  /* Team — cyan accent */
  .badge-team {
    background: var(--cyan-soft);
    color: var(--cyan);
    border: 1px solid rgba(0, 212, 255, 0.28);
  }

  /* Enterprise — ember with stronger presence */
  .badge-enterprise {
    background: var(--ember-soft);
    color: var(--ember);
    border: 1px solid var(--ember-glow);
    font-weight: 700;
  }

  .degraded {
    opacity: 0.55;
    filter: grayscale(0.4);
  }
</style>
