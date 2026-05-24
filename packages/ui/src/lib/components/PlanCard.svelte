<svelte:options runes={true} />
<script lang="ts">
  import { Layers, Zap, Users, Building2, Check } from '@lucide/svelte';

  let {
    planKey,
    displayName,
    monthlyPriceCents = 0,
    maxTurnsPerDay = null,
    maxStorageGb = null,
    maxTokens = 0,
    rateLimitRpm = 0,
    current = false,
    onUpgrade = null,
  }: {
    planKey: string;
    displayName: string;
    monthlyPriceCents?: number;
    maxTurnsPerDay?: number | null;
    maxStorageGb?: number | null;
    maxTokens?: number;
    rateLimitRpm?: number;
    current?: boolean;
    onUpgrade?: (() => void) | null;
  } = $props();

  const isEnterprise = $derived(planKey === 'enterprise');
  const priceLabel = $derived(
    monthlyPriceCents === 0 ? 'Free' : `$${(monthlyPriceCents / 100).toFixed(0)}`
  );
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
    <button class="btn-upgrade" onclick={() => onUpgrade?.()}>
      {monthlyPriceCents > 0 ? 'Upgrade' : 'Downgrade'}
    </button>
  {/if}
</div>

<style>
  .plan-card {
    padding:    var(--space-5);
    border:     1px solid var(--color-border);
    border-radius: var(--radius-lg);
    display:    flex;
    flex-direction: column;
    gap:        var(--space-3);
    background: var(--color-bg);
    transition:
      box-shadow   var(--duration-normal) var(--ease-standard),  /* [feedback] */
      border-color var(--duration-normal) var(--ease-standard);
  }

  .plan-card.current {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-accent) 20%, transparent);
  }

  .plan-card:not(.current):hover {
    box-shadow: 0 8px 24px var(--color-shadow-md, rgba(17,17,17,0.08));
  }

  .plan-header {
    display:        flex;
    flex-direction: column;
    gap:            0.3rem;
  }

  .plan-icon {
    display:       inline-flex;
    color:         var(--color-accent);
    margin-bottom: 0.15rem;
  }

  .plan-header h3 {
    font-family:    var(--font-family-sans);
    font-weight:    700;
    font-size:      var(--font-size-body);
    letter-spacing: -0.02em;
    margin:         0;
    color:          var(--color-fg);
  }

  .price {
    display:     flex;
    align-items: baseline;
    gap:         0.2rem;
  }

  .price strong {
    font-family:    var(--font-family-sans);
    font-size:      var(--font-size-display, 1.5rem);
    font-weight:    800;
    letter-spacing: -0.04em;
    color:          var(--color-fg);
  }

  .per-mo {
    font-size: var(--font-size-label);
    color:     var(--color-fg-subtle);
  }

  .features {
    list-style:     none;
    padding:        0;
    margin:         0;
    font-size:      var(--font-size-meta);
    color:          var(--color-fg-muted);
    flex:           1;
    display:        flex;
    flex-direction: column;
    gap:            0.3rem;
  }

  .features li {
    display:     flex;
    align-items: center;
    gap:         0.4rem;
    padding:     0.1rem 0;
  }

  .features li :global(svg) {
    color:       var(--color-success, var(--color-accent));
    flex-shrink: 0;
  }

  .current-tag {
    text-align:     center;
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color:          var(--color-accent);
    font-weight:    600;
    padding:        0.4rem;
    border-top:     1px solid var(--color-accent-soft);
  }

  .btn-upgrade {
    display:       block;
    width:         100%;
    padding:       0.55rem;
    background:    var(--color-accent);
    color:         var(--color-on-accent);
    border:        none;
    border-radius: var(--radius-md);
    font-family:   var(--font-family-sans);
    font-weight:   600;
    font-size:     var(--font-size-meta);
    cursor:        pointer;
    transition:
      transform  var(--duration-fast) var(--ease-standard),  /* [feedback] */
      box-shadow var(--duration-fast) var(--ease-standard);
    box-shadow: 0 4px 14px color-mix(in srgb, var(--color-accent) 30%, transparent);
  }

  .btn-upgrade:hover {
    transform:  translateY(-2px);
    box-shadow: 0 8px 20px color-mix(in srgb, var(--color-accent) 35%, transparent);
  }

  .btn-upgrade:active {
    transform: scale(0.97);
  }

  .btn-contact {
    display:       block;
    text-align:    center;
    padding:       0.55rem;
    background:    transparent;
    color:         var(--color-fg-muted);
    border:        1px solid var(--color-border);
    border-radius: var(--radius-md);
    text-decoration: none;
    font-size:     var(--font-size-meta);
    font-weight:   600;
    transition:    background var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }

  .btn-contact:hover {
    background: var(--color-bg-raised);
  }
</style>
