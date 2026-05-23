<svelte:options runes={true} />
<script lang="ts">
  /**
   * Account page — Phase 4.4
   * Profile card + nav links. Consumes PlanBadge + Button primitives.
   * Local CSS replaced with semantic design tokens.
   */
  import { CreditCard, BarChart3, LogOut } from 'lucide-svelte';
  import { PlanBadge, Button, Icon } from '@conusai/ui';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  const { user, subscription, authProvider } = data;

  const planLabel   = $derived(subscription?.plan_key ?? user?.plan ?? 'free');
  const statusLabel = $derived(subscription?.status ?? 'active');
  const showStatus  = $derived(statusLabel !== 'active' && statusLabel !== 'trialing');
  const logoutHref  = $derived(authProvider === 'zitadel' ? '/auth/logout' : '/logout');
</script>

<svelte:head>
  <title>Account — ConusAI</title>
</svelte:head>

<div class="account-page">

  <!-- Page header -->
  <header class="page-header">
    <p class="page-eyebrow">Settings</p>
    <h1 class="page-title">Account</h1>
  </header>

  <!-- Profile card -->
  <section class="profile-card" aria-label="Profile">
    <div class="avatar" aria-label="User avatar: {(user?.name ?? '?')[0].toUpperCase()}">
      {(user?.name ?? '?')[0].toUpperCase()}
    </div>
    <div class="profile-info">
      <p class="profile-name">{user?.name ?? 'Unknown'}</p>
      <div class="profile-badges">
        <PlanBadge plan={planLabel} />
        {#if showStatus}
          <span class="status-pill status-{statusLabel}">
            {statusLabel.replace('_', ' ')}
          </span>
        {/if}
      </div>
    </div>
  </section>

  <!-- Nav links -->
  <nav class="account-links" aria-label="Account navigation">
    <a href="/account/billing" class="link-card">
      <span class="link-icon" aria-hidden="true">
        <Icon icon={CreditCard} size="md" />
      </span>
      <div class="link-body">
        <strong class="link-title">Billing &amp; Plans</strong>
        <p class="link-desc">Manage your subscription, upgrade, or view invoices.</p>
      </div>
      <span class="link-arrow" aria-hidden="true">›</span>
    </a>

    <a href="/account/usage" class="link-card">
      <span class="link-icon" aria-hidden="true">
        <Icon icon={BarChart3} size="md" />
      </span>
      <div class="link-body">
        <strong class="link-title">Usage</strong>
        <p class="link-desc">View agent turns, token consumption, and storage.</p>
      </div>
      <span class="link-arrow" aria-hidden="true">›</span>
    </a>

    <a href={logoutHref} class="link-card link-destructive">
      <span class="link-icon" aria-hidden="true">
        <Icon icon={LogOut} size="md" />
      </span>
      <div class="link-body">
        <strong class="link-title">Sign out</strong>
        <p class="link-desc">End your session.</p>
      </div>
      <span class="link-arrow" aria-hidden="true">›</span>
    </a>
  </nav>

</div>

<style>
  /* ── Page layout ─────────────────────────────────────────────────────────── */
  .account-page {
    max-width: 600px;
    margin:    0 auto;
    padding:   var(--space-7) var(--space-4);
  }

  /* ── Page header ─────────────────────────────────────────────────────────── */
  .page-header {
    margin-bottom: var(--space-6);
  }

  .page-eyebrow {
    margin:         0 0 var(--space-1);
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color:          var(--color-fg-subtle);
  }

  .page-title {
    margin:         0;
    font-size:      var(--font-size-h1);     /* 28px */
    font-weight:    620;
    letter-spacing: -0.025em;
    color:          var(--color-fg);
    line-height:    1.2;
  }

  /* ── Profile card ────────────────────────────────────────────────────────── */
  .profile-card {
    display:       flex;
    align-items:   center;
    gap:           var(--space-4);
    padding:       var(--space-4) var(--space-5);
    border:        1px solid var(--color-border);
    border-radius: var(--radius-lg);
    margin-bottom: var(--space-4);
    background:    var(--color-bg-raised);
  }

  .avatar {
    width:          48px;
    height:         48px;
    border-radius:  var(--radius-full);
    background:     var(--color-accent);
    color:          #fff;
    display:        flex;
    align-items:    center;
    justify-content: center;
    font-weight:    700;
    font-size:      18px;
    letter-spacing: -0.02em;
    flex-shrink:    0;
    user-select:    none;
  }

  .profile-info {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-2);
  }

  .profile-name {
    margin:      0;
    font-weight: 550;
    font-size:   var(--font-size-body);
    color:       var(--color-fg);
  }

  .profile-badges {
    display:     flex;
    align-items: center;
    gap:         var(--space-2);
    flex-wrap:   wrap;
  }

  .status-pill {
    display:        inline-flex;
    align-items:    center;
    padding:        2px var(--space-2);
    border-radius:  var(--radius-full);
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);
    font-weight:    600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    background:     var(--color-danger-soft);
    color:          var(--color-danger);
    border:         1px solid var(--color-danger);
  }

  /* ── Nav links ───────────────────────────────────────────────────────────── */
  .account-links {
    display:        flex;
    flex-direction: column;
    gap:            var(--space-2);
  }

  .link-card {
    display:       flex;
    align-items:   center;
    gap:           var(--space-4);
    padding:       var(--space-4) var(--space-5);
    border:        1px solid var(--color-border);
    border-radius: var(--radius-lg);
    text-decoration: none;
    color:         inherit;
    background:    var(--color-bg-raised);
    min-height:    var(--hit);

    transition:
      background   var(--duration-fast) var(--ease-standard),
      border-color var(--duration-fast) var(--ease-standard),
      box-shadow   var(--duration-fast) var(--ease-standard);
  }

  .link-card:hover {
    background:   var(--color-bg-hover);
    border-color: var(--color-border-strong);
    box-shadow:   0 2px 8px var(--color-shadow-sm);
  }

  .link-card:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  /* Icon pip */
  .link-icon {
    display:        flex;
    align-items:    center;
    justify-content: center;
    width:          36px;
    height:         36px;
    border-radius:  var(--radius-sm);
    background:     var(--color-accent-soft);
    color:          var(--color-accent);
    flex-shrink:    0;
  }

  /* Body */
  .link-body {
    flex: 1;
    min-width: 0;
  }

  .link-title {
    display:        block;
    font-weight:    550;
    font-size:      var(--font-size-body);
    color:          var(--color-fg);
    margin-bottom:  2px;
  }

  .link-desc {
    margin:      0;
    font-size:   var(--font-size-meta);
    color:       var(--color-fg-subtle);
    line-height: 1.45;
  }

  /* Arrow */
  .link-arrow {
    font-size:   18px;
    color:       var(--color-fg-subtle);
    flex-shrink: 0;
  }

  /* Destructive */
  .link-destructive {
    border-color: var(--color-danger-soft);
  }
  .link-destructive .link-icon {
    background: var(--color-danger-soft);
    color:      var(--color-danger);
  }
  .link-destructive:hover {
    background:   var(--color-danger-soft);
    border-color: var(--color-danger);
  }
  .link-destructive .link-title { color: var(--color-danger); }

  @media (prefers-reduced-motion: reduce) {
    .link-card { transition: none; }
  }
</style>
