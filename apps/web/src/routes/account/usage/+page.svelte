<svelte:options runes={true} />
<script lang="ts">
  /**
   * Usage page — Phase 4.6
   * Agent turns, tokens, storage meters. Consumes UsageMeter + Button primitives.
   * Local CSS replaced with semantic tokens.
   */
  import { ArrowUpRight } from '@lucide/svelte';
  import { UsageMeter, Button, PageHeader, Breadcrumbs } from '@conusai/ui';
  import type { PageData } from './$types.js';

  let { data }: { data: PageData } = $props();
  const { usage, subscription } = data;

  const planKey = $derived(subscription?.plan_key ?? 'free');

  const limits: Record<string, { turns: number | null; tokens: number | null }> = {
    free:       { turns: 50,   tokens: null },
    pro:        { turns: 500,  tokens: null },
    team:       { turns: 2000, tokens: null },
    enterprise: { turns: null, tokens: null },
  };

  const limit = $derived(limits[planKey] ?? limits.free);

  function fmt(n: number): string {
    return n.toLocaleString();
  }
</script>

<svelte:head>
  <title>Usage — ConusAI</title>
</svelte:head>

<div class="usage-page">

  <Breadcrumbs items={[{ label: 'Account', href: '/account' }, { label: 'Usage' }]} />

  <PageHeader eyebrow="Usage" title="Usage" subtitle="Today (UTC)" />

  <!-- Meter cards -->
  <div class="meters">

    <!-- Agent Turns -->
    <article class="meter-card" aria-label="Agent turns usage">
      <header class="meter-header">
        <span class="meter-label">Agent Turns</span>
        <span class="meter-value" aria-label="{fmt(usage.agent_turns)}{limit.turns ? ` of ${fmt(limit.turns)}` : ''} turns">
          {fmt(usage.agent_turns)}{limit.turns ? ` / ${fmt(limit.turns)}` : ''}
        </span>
      </header>
      {#if limit.turns}
        <UsageMeter
          used={usage.agent_turns}
          limit={limit.turns}
          label="Agent turns used"
        />
        <p class="meter-hint">
          {fmt(limit.turns - usage.agent_turns)} remaining today
        </p>
      {:else}
        <p class="meter-hint meter-unlimited">Unlimited on your plan</p>
      {/if}
    </article>

    <!-- Tokens -->
    <article class="meter-card" aria-label="Token usage">
      <header class="meter-header">
        <span class="meter-label">Tokens Used</span>
        <span class="meter-value">{fmt(usage.tokens)}</span>
      </header>
      <p class="meter-hint">Billed as usage — see invoices for breakdown</p>
    </article>

    <!-- Storage -->
    <article class="meter-card" aria-label="Storage usage">
      <header class="meter-header">
        <span class="meter-label">Storage</span>
        <span class="meter-value">{usage.storage_gb.toFixed(2)} GB</span>
      </header>
      <p class="meter-hint">Workspace files and artifacts</p>
    </article>

  </div>

  <!-- Upgrade banner (free tier only) -->
  {#if planKey === 'free'}
    <aside class="upgrade-banner" aria-label="Upgrade prompt">
      <p class="upgrade-text">
        You're on the Free plan. Upgrade for more turns, tokens, and storage.
      </p>
      <a href="/account/billing" class="upgrade-link">
        Upgrade Now
        <ArrowUpRight size={15} strokeWidth={1.75} aria-hidden="true" />
      </a>
    </aside>
  {/if}

</div>

<style>
  /* ── Page ────────────────────────────────────────────────────────────────── */
  .usage-page {
    max-width: 640px;
    margin:    0 auto;
    padding:   var(--space-7) var(--space-4);
    display:   flex;
    flex-direction: column;
    gap:       var(--space-5);
  }


  /* ── Meter cards ─────────────────────────────────────────────────────────── */
  .meters {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-3);
  }

  .meter-card {
    padding:       var(--space-5);
    border:        1px solid var(--color-border);
    border-radius: var(--radius-lg);
    background:    var(--color-bg-raised);
    display:       flex;
    flex-direction: column;
    gap:           var(--space-3);
  }

  .meter-header {
    display:     flex;
    justify-content: space-between;
    align-items: baseline;
    gap:         var(--space-3);
  }

  .meter-label {
    font-weight: 580;
    font-size:   var(--font-size-body);
    color:       var(--color-fg);
  }

  .meter-value {
    font-size:      18px;
    font-weight:    700;
    letter-spacing: -0.03em;
    color:          var(--color-fg);
    font-variant-numeric: tabular-nums;
  }

  .meter-hint {
    margin:         0;
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    letter-spacing: 0.03em;
    color:          var(--color-fg-subtle);
  }

  .meter-unlimited { color: var(--color-success); }

  /* ── Upgrade banner ──────────────────────────────────────────────────────── */
  .upgrade-banner {
    padding:       var(--space-5) var(--space-5);
    background:    var(--color-accent-soft);
    border:        1px solid var(--color-accent);
    border-radius: var(--radius-lg);
    display:       flex;
    align-items:   center;
    justify-content: space-between;
    gap:           var(--space-4);
    flex-wrap:     wrap;
  }

  .upgrade-text {
    margin:      0;
    font-size:   var(--font-size-meta);
    color:       var(--color-fg-muted);
    font-weight: 500;
  }

  .upgrade-link {
    display:         inline-flex;
    align-items:     center;
    gap:             var(--space-1);
    padding:         var(--space-2) var(--space-4);
    background:      var(--color-accent);
    color:           var(--color-on-accent);
    border-radius:   var(--radius-md);
    font-weight:     600;
    font-size:       var(--font-size-meta);
    text-decoration: none;
    white-space:     nowrap;
    transition:      filter var(--duration-fast) var(--ease-standard);
  }
  .upgrade-link:hover { filter: brightness(1.08); }

  .upgrade-link:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  @media (prefers-reduced-motion: reduce) {
    .upgrade-link { transition: none; }
  }
</style>
