<svelte:options runes={true} />
<script lang="ts">
  /**
   * Billing page — Phase 4.5
   * Plan cards + invoices. Consumes PlanBadge, StatusBadge, Button primitives.
   * Local CSS replaced with design-system tokens; raw buttons replaced with Button.
   */
  import { enhance } from '$app/forms';
  import { Layers, Zap, Users, Building2, Check, ArrowUpRight } from 'lucide-svelte';
  import { PlanBadge, StatusBadge, Button } from '@conusai/ui';
  import type { ActionData, PageData } from './$types';

  let { data, form }: { data: PageData; form: ActionData } = $props();

  const { plans, subscription } = data;

  const currentPlan = $derived(subscription?.plan_key ?? 'free');
  const isActive    = $derived(
    subscription?.status === 'active' || subscription?.status === 'trialing'
  );

  const subscriptionStatus = $derived((): import('@conusai/ui').StatusKind => {
    switch (subscription?.status) {
      case 'active':
      case 'trialing': return 'success';
      case 'past_due': return 'warning';
      case 'canceled': return 'danger';
      default:         return 'neutral';
    }
  });

  const planIcons: Record<string, typeof Zap> = {
    free: Layers, pro: Zap, team: Users, enterprise: Building2,
  };
</script>

<svelte:head>
  <title>Billing — ConusAI</title>
</svelte:head>

<div class="billing-page">

  <!-- Breadcrumb -->
  <nav class="breadcrumb" aria-label="Breadcrumb">
    <a href="/account">Account</a>
    <span aria-hidden="true">›</span>
    <span aria-current="page">Billing</span>
  </nav>

  <header class="page-header">
    <h1 class="page-title">Billing &amp; Plans</h1>
  </header>

  {#if form?.error}
    <p class="error-banner" role="alert">{form.error}</p>
  {/if}

  <!-- ── Current plan ─────────────────────────────────────────────── -->
  {#if subscription}
    <section class="current-plan" aria-label="Current plan">
      <h2 class="section-heading">Current Plan</h2>
      <div class="plan-summary">
        <PlanBadge plan={currentPlan} />
        <StatusBadge
          status={subscriptionStatus()}
          label={subscription.status.replace('_', ' ')}
        />
        {#if subscription.current_period_end}
          <span class="period-text">
            Renews {new Date(subscription.current_period_end).toLocaleDateString()}
          </span>
        {/if}
      </div>
      <div class="portal-actions">
        <form method="POST" action="?/portal" use:enhance>
          <Button type="submit" variant="secondary" size="sm" text="Manage Billing" />
        </form>
        {#if isActive && currentPlan !== 'free'}
          <form method="POST" action="?/cancel" use:enhance>
            <Button type="submit" variant="danger" size="sm" text="Cancel Subscription" />
          </form>
        {/if}
      </div>
    </section>
  {/if}

  <!-- ── Plan cards ───────────────────────────────────────────────── -->
  <section class="plans-section" aria-label="Available plans">
    <h2 class="section-heading">Available Plans</h2>
    <div class="plans-grid">
      {#each plans as plan (plan.key)}
        {@const isCurrent   = plan.key === currentPlan}
        {@const PlanIcon    = planIcons[plan.key] ?? Layers}
        <article class="plan-card" class:current={isCurrent} aria-label="{plan.display_name} plan">
          <header class="plan-header">
            <span class="plan-icon" aria-hidden="true">
              <PlanIcon size={20} strokeWidth={1.5} />
            </span>
            <h3 class="plan-name">{plan.display_name}</h3>
            <div class="plan-price">
              {#if plan.monthly_price_cents === 0}
                <strong class="price-amount">Free</strong>
              {:else}
                <strong class="price-amount">${(plan.monthly_price_cents / 100).toFixed(0)}</strong>
                <span class="price-unit">/mo</span>
              {/if}
            </div>
          </header>

          <ul class="feature-list">
            {#if plan.max_turns_per_day}
              <li><Check size={12} strokeWidth={2} aria-hidden="true" />{plan.max_turns_per_day.toLocaleString()} agent turns/day</li>
            {:else}
              <li><Check size={12} strokeWidth={2} aria-hidden="true" />Unlimited agent turns</li>
            {/if}
            {#if plan.max_storage_gb}
              <li><Check size={12} strokeWidth={2} aria-hidden="true" />{plan.max_storage_gb} GB storage</li>
            {:else}
              <li><Check size={12} strokeWidth={2} aria-hidden="true" />Unlimited storage</li>
            {/if}
            <li><Check size={12} strokeWidth={2} aria-hidden="true" />{plan.max_tokens.toLocaleString()} tokens/request</li>
            <li><Check size={12} strokeWidth={2} aria-hidden="true" />{plan.rate_limit_rpm} requests/min</li>
          </ul>

          {#if plan.key !== currentPlan && plan.key !== 'enterprise'}
            <form method="POST" action="?/upgrade" use:enhance>
              <input type="hidden" name="plan_key" value={plan.key} />
              <Button
                type="submit"
                variant="primary"
                size="sm"
                fullWidth
                text={plan.monthly_price_cents > 0 ? 'Upgrade' : 'Downgrade'}
                iconTrailing={ArrowUpRight}
              />
            </form>
          {:else if isCurrent}
            <div class="current-badge" aria-label="Your current plan">Current Plan</div>
          {:else}
            <a href="mailto:sales@conusai.com" class="contact-link">Contact Sales</a>
          {/if}
        </article>
      {/each}
    </div>
  </section>

  <!-- ── Invoices ──────────────────────────────────────────────────── -->
  <section class="invoices-section" aria-label="Invoices">
    <h2 class="section-heading">Invoices</h2>
    <p class="invoices-hint">View and download invoices from the billing portal.</p>
    <form method="POST" action="?/portal" use:enhance>
      <Button type="submit" variant="ghost" size="sm" text="Open billing portal" />
    </form>
  </section>

</div>

<style>
  /* ── Page ────────────────────────────────────────────────────────────────── */
  .billing-page {
    max-width: 820px;
    margin:    0 auto;
    padding:   var(--space-7) var(--space-4);
    display:   flex;
    flex-direction: column;
    gap:       var(--space-6);
  }

  /* ── Breadcrumb ──────────────────────────────────────────────────────────── */
  .breadcrumb {
    display:     flex;
    gap:         var(--space-2);
    align-items: center;
    font-family: var(--font-family-mono);
    font-size:   var(--font-size-meta);
    color:       var(--color-fg-subtle);
  }
  .breadcrumb a {
    color:           var(--color-accent);
    text-decoration: none;
    font-weight:     500;
  }
  .breadcrumb a:hover { text-decoration: underline; }

  /* ── Page header ─────────────────────────────────────────────────────────── */
  .page-header { margin-bottom: calc(var(--space-5) * -1); }

  .page-title {
    margin:         0;
    font-size:      var(--font-size-h1);
    font-weight:    620;
    letter-spacing: -0.025em;
    color:          var(--color-fg);
  }

  /* ── Section headings ────────────────────────────────────────────────────── */
  .section-heading {
    margin:         0 0 var(--space-3);
    font-size:      var(--font-size-body);
    font-weight:    580;
    letter-spacing: -0.01em;
    color:          var(--color-fg);
  }

  /* ── Error banner ────────────────────────────────────────────────────────── */
  .error-banner {
    margin:        0;
    background:    var(--color-danger-soft);
    color:         var(--color-danger);
    padding:       var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    border:        1px solid var(--color-danger);
    font-size:     var(--font-size-meta);
  }

  /* ── Current plan ────────────────────────────────────────────────────────── */
  .current-plan {
    padding:       var(--space-5);
    border:        1px solid var(--color-border);
    border-radius: var(--radius-lg);
    background:    var(--color-bg-raised);
  }

  .plan-summary {
    display:     flex;
    gap:         var(--space-3);
    align-items: center;
    margin-bottom: var(--space-4);
    flex-wrap:   wrap;
  }

  .period-text {
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-meta);
    color:          var(--color-fg-subtle);
    letter-spacing: 0.03em;
  }

  .portal-actions {
    display: flex;
    gap:     var(--space-3);
    flex-wrap: wrap;
  }

  /* ── Plans grid ──────────────────────────────────────────────────────────── */
  .plans-grid {
    display:               grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap:                   var(--space-4);
  }

  /* ── Plan card ───────────────────────────────────────────────────────────── */
  .plan-card {
    padding:        var(--space-5);
    border:         1px solid var(--color-border);
    border-radius:  var(--radius-lg);
    display:        flex;
    flex-direction: column;
    gap:            var(--space-3);
    background:     var(--color-bg-raised);

    transition: box-shadow var(--duration-fast) var(--ease-standard);
  }

  .plan-card.current {
    border-color: var(--color-accent);
    box-shadow:   0 0 0 2px var(--color-accent-soft);
  }

  .plan-card:not(.current):hover {
    box-shadow: 0 4px 16px var(--color-shadow-sm);
  }

  .plan-header {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-1);
  }

  .plan-icon {
    display: inline-flex;
    color:   var(--color-accent);
    margin-bottom: 2px;
  }

  .plan-name {
    margin:         0;
    font-size:      var(--font-size-body);
    font-weight:    580;
    letter-spacing: -0.012em;
    color:          var(--color-fg);
  }

  .plan-price {
    display:     flex;
    align-items: baseline;
    gap:         2px;
  }

  .price-amount {
    font-size:      22px;
    font-weight:    700;
    letter-spacing: -0.04em;
    color:          var(--color-fg);
  }

  .price-unit {
    font-size: var(--font-size-meta);
    color:     var(--color-fg-subtle);
  }

  .feature-list {
    list-style: none;
    padding:    0;
    margin:     0;
    font-size:  var(--font-size-meta);
    color:      var(--color-fg-muted);
    flex:       1;
    display:    flex;
    flex-direction: column;
    gap:        var(--space-1);
  }

  .feature-list li {
    display:     flex;
    align-items: center;
    gap:         var(--space-2);
  }

  .feature-list li :global(svg) {
    color:       var(--color-success);
    flex-shrink: 0;
  }

  /* Current plan badge */
  .current-badge {
    text-align:     center;
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color:          var(--color-accent);
    font-weight:    600;
    padding:        var(--space-2) 0;
    border-top:     1px solid var(--color-accent-soft);
  }

  /* Contact sales link */
  .contact-link {
    display:         block;
    text-align:      center;
    padding:         var(--space-2) var(--space-3);
    border:          1px solid var(--color-border);
    border-radius:   var(--radius-sm);
    text-decoration: none;
    font-size:       var(--font-size-meta);
    font-weight:     500;
    color:           var(--color-fg-muted);
    transition:      background var(--duration-fast) var(--ease-standard);
  }
  .contact-link:hover { background: var(--color-bg-hover); }

  /* ── Invoices ────────────────────────────────────────────────────────────── */
  .invoices-hint {
    margin:      0 0 var(--space-3);
    font-size:   var(--font-size-meta);
    color:       var(--color-fg-subtle);
  }

  @media (prefers-reduced-motion: reduce) {
    .plan-card { transition: none; }
  }
</style>
