<script lang="ts">
  import { Layers, Zap, Users, Building2, Check } from 'lucide-svelte';

  export let planKey: string;
  export let displayName: string;
  export let monthlyPriceCents: number = 0;
  export let maxTurnsPerDay: number | null = null;
  export let maxStorageGb: number | null = null;
  export let maxTokens: number = 0;
  export let rateLimitRpm: number = 0;
  export let current: boolean = false;
  export let onUpgrade: (() => void) | null = null;

  $: isEnterprise = planKey === 'enterprise';
  $: priceLabel = monthlyPriceCents === 0 ? 'Free' : `$${(monthlyPriceCents / 100).toFixed(0)}`;
</script>

<div class="plan-card" class:current>
  <div class="plan-header">
    <span class="plan-icon" aria-hidden="true">
      {#if planKey === 'pro'}
        <Zap size={20} strokeWidth={1.5} />
      {:else if planKey === 'team'}
        <Users size={20} strokeWidth={1.5} />
      {:else if planKey === 'enterprise'}
        <Building2 size={20} strokeWidth={1.5} />
      {:else}
        <Layers size={20} strokeWidth={1.5} />
      {/if}
    </span>
    <h3>{displayName}</h3>
    <div class="price">
      <strong>{priceLabel}</strong>
      {#if monthlyPriceCents > 0}<span class="per-mo">/mo</span>{/if}
    </div>
  </div>

  <ul class="features">
    {#if maxTurnsPerDay !== null}
      <li><Check size={13} strokeWidth={2} aria-hidden="true" />{maxTurnsPerDay.toLocaleString()} agent turns/day</li>
    {:else}
      <li><Check size={13} strokeWidth={2} aria-hidden="true" />Unlimited agent turns</li>
    {/if}
    {#if maxStorageGb !== null}
      <li><Check size={13} strokeWidth={2} aria-hidden="true" />{maxStorageGb} GB storage</li>
    {:else}
      <li><Check size={13} strokeWidth={2} aria-hidden="true" />Unlimited storage</li>
    {/if}
    <li><Check size={13} strokeWidth={2} aria-hidden="true" />{maxTokens.toLocaleString()} max tokens/request</li>
    <li><Check size={13} strokeWidth={2} aria-hidden="true" />{rateLimitRpm} requests/min</li>
  </ul>

  {#if current}
    <div class="current-tag">Current Plan</div>
  {:else if isEnterprise}
    <a href="mailto:sales@conusai.com" class="btn-contact">Contact Sales</a>
  {:else if onUpgrade}
    <button class="btn-upgrade" on:click={onUpgrade}>
      {monthlyPriceCents > 0 ? 'Upgrade' : 'Downgrade'}
    </button>
  {/if}
</div>

<style>
  .plan-card {
    padding: 1.25rem;
    border: 1px solid var(--rule);
    border-radius: var(--r-lg);
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    background: var(--paper);
    transition: box-shadow 180ms cubic-bezier(0.4, 0, 0.2, 1),
                border-color 180ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  .plan-card.current {
    border-color: var(--ember);
    box-shadow: 0 0 0 2px var(--ember-glow);
  }

  .plan-card:not(.current):hover {
    box-shadow: 0 8px 24px rgba(17, 17, 17, 0.08);
  }

  .plan-header {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .plan-icon {
    display: inline-flex;
    color: var(--ember);
    margin-bottom: 0.15rem;
  }

  .plan-header h3 {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 1rem;
    letter-spacing: -0.02em;
    margin: 0;
    color: var(--ink);
  }

  .price {
    display: flex;
    align-items: baseline;
    gap: 0.2rem;
  }

  .price strong {
    font-family: var(--font-display);
    font-size: 1.5rem;
    font-weight: 800;
    letter-spacing: -0.04em;
    color: var(--ink);
  }

  .per-mo {
    font-size: 0.8rem;
    color: var(--ink-3);
  }

  .features {
    list-style: none;
    padding: 0;
    margin: 0;
    font-size: 0.825rem;
    color: var(--ink-2);
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .features li {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.1rem 0;
  }

  .features li :global(svg) {
    color: var(--success);
    flex-shrink: 0;
  }

  .current-tag {
    text-align: center;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ember);
    font-weight: 600;
    padding: 0.4rem;
    border-top: 1px solid var(--ember-soft);
  }

  .btn-upgrade {
    display: block;
    width: 100%;
    padding: 0.55rem;
    background: var(--ember);
    color: #fff;
    border: none;
    border-radius: var(--r-md);
    font-family: var(--font-body);
    font-weight: 600;
    font-size: 0.875rem;
    cursor: pointer;
    transition: transform 120ms cubic-bezier(0.4, 0, 0.2, 1),
                box-shadow 120ms cubic-bezier(0.4, 0, 0.2, 1);
    box-shadow: 0 4px 14px var(--ember-glow);
  }

  .btn-upgrade:hover {
    transform: translateY(-2px);
    box-shadow: 0 8px 20px var(--ember-glow);
  }

  .btn-upgrade:active {
    transform: scale(0.97);
  }

  .btn-contact {
    display: block;
    text-align: center;
    padding: 0.55rem;
    background: transparent;
    color: var(--ink-2);
    border: 1px solid var(--seam);
    border-radius: var(--r-md);
    text-decoration: none;
    font-size: 0.875rem;
    font-weight: 600;
    transition: background 120ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  .btn-contact:hover {
    background: var(--paper-2);
  }
</style>
