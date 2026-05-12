<script lang="ts">
  import type { ComponentEvents } from 'svelte';

  export let planKey: string;
  export let displayName: string;
  export let monthlyPriceCents: number = 0;
  export let maxTurnsPerDay: number | null = null;
  export let maxStorageGb: number | null = null;
  export let maxTokens: number = 0;
  export let rateLimitRpm: number = 0;
  export let current: boolean = false;
  export let onUpgrade: (() => void) | null = null;

  const icons: Record<string, string> = {
    free: '🆓', pro: '⚡', team: '👥', enterprise: '🏢',
  };

  $: icon = icons[planKey] ?? '📦';
  $: isEnterprise = planKey === 'enterprise';
  $: priceLabel = monthlyPriceCents === 0 ? 'Free' : `$${(monthlyPriceCents / 100).toFixed(0)}`;
</script>

<div class="plan-card" class:current>
  <div class="plan-header">
    <span class="plan-icon">{icon}</span>
    <h3>{displayName}</h3>
    <div class="price">
      <strong>{priceLabel}</strong>
      {#if monthlyPriceCents > 0}<span>/mo</span>{/if}
    </div>
  </div>

  <ul class="features">
    {#if maxTurnsPerDay !== null}
      <li>{maxTurnsPerDay.toLocaleString()} agent turns/day</li>
    {:else}
      <li>Unlimited agent turns</li>
    {/if}
    {#if maxStorageGb !== null}
      <li>{maxStorageGb} GB storage</li>
    {:else}
      <li>Unlimited storage</li>
    {/if}
    <li>{maxTokens.toLocaleString()} max tokens/request</li>
    <li>{rateLimitRpm} requests/min</li>
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
    border: 1px solid #e5e7eb;
    border-radius: 12px;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    background: #fff;
  }
  .plan-card.current { border-color: #6366f1; box-shadow: 0 0 0 2px #c7d2fe; }
  .plan-header { display: flex; flex-direction: column; gap: 0.25rem; }
  .plan-icon { font-size: 1.5rem; }
  .plan-header h3 { font-weight: 600; margin: 0; font-size: 1rem; }
  .price { display: flex; align-items: baseline; gap: 0.25rem; }
  .price strong { font-size: 1.5rem; font-weight: 700; }
  .price span { color: #6b7280; font-size: 0.875rem; }
  .features { list-style: none; padding: 0; margin: 0; font-size: 0.85rem; color: #374151; flex: 1; }
  .features li::before { content: '✓ '; color: #16a34a; }
  .features li { padding: 0.15rem 0; }
  .current-tag { text-align: center; font-size: 0.8rem; color: #6366f1; font-weight: 600; padding: 0.375rem; }
  .btn-upgrade {
    display: block; width: 100%; padding: 0.5rem;
    background: #6366f1; color: #fff; border: none;
    border-radius: 8px; font-weight: 600; cursor: pointer;
    font-size: 0.875rem;
  }
  .btn-upgrade:hover { background: #4f46e5; }
  .btn-contact {
    display: block; text-align: center; padding: 0.5rem;
    background: #f3f4f6; color: #374151;
    border: 1px solid #d1d5db; border-radius: 8px;
    text-decoration: none; font-size: 0.875rem;
  }
  .btn-contact:hover { background: #e5e7eb; }
</style>
