<svelte:options runes={true} />
<script lang="ts">
  import { onDestroy } from 'svelte';
  import { AlertTriangle, Ban, Check, X, ArrowUpRight } from 'lucide-svelte';

  let {
    apiBase = '',
    upgradeUrl = '/account/billing',
  }: {
    apiBase?: string;
    upgradeUrl?: string;
  } = $props();

  type BannerKind = 'quota_warning' | 'quota_exceeded' | 'subscription_updated';

  interface BannerEvent {
    kind: BannerKind;
    message?: string;
    plan?: string;
  }

  let banner = $state<BannerEvent | null>(null);
  let es: EventSource | null = null;

  function connect() {
    if (typeof EventSource === 'undefined') return;
    es = new EventSource(`${apiBase}/v1/realtime`);

    es.addEventListener('quota.warning', () => {
      banner = { kind: 'quota_warning', message: 'Approaching your daily quota.' };
    });

    es.addEventListener('quota.exceeded', () => {
      banner = { kind: 'quota_exceeded', message: 'Daily quota reached. Upgrade to continue.' };
    });

    es.addEventListener('subscription.updated', (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data);
        banner = { kind: 'subscription_updated', plan: data.plan_tier, message: 'Your plan has been updated.' };
        setTimeout(() => { if (banner?.kind === 'subscription_updated') banner = null; }, 5000);
      } catch { /* ignore parse errors */ }
    });
  }

  connect();
  onDestroy(() => es?.close());

  function dismiss() { banner = null; }
</script>

{#if banner}
  <div class="quota-banner banner-{banner.kind}" role="alert" aria-live="assertive">
    <span class="banner-body">
      <span class="banner-icon" aria-hidden="true">
        {#if banner.kind === 'quota_exceeded'}
          <Ban size={15} strokeWidth={1.75} />
        {:else if banner.kind === 'quota_warning'}
          <AlertTriangle size={15} strokeWidth={1.75} />
        {:else}
          <Check size={15} strokeWidth={1.75} />
        {/if}
      </span>
      <span class="banner-msg">{banner.message ?? ''}</span>
    </span>

    <div class="banner-actions">
      {#if banner.kind !== 'subscription_updated'}
        <a href={upgradeUrl} class="btn-upgrade">
          Upgrade
          <ArrowUpRight size={13} strokeWidth={1.75} aria-hidden="true" />
        </a>
      {/if}
      <button class="btn-dismiss" onclick={dismiss} aria-label="Dismiss notification">
        <X size={14} strokeWidth={1.75} />
      </button>
    </div>
  </div>
{/if}

<style>
  .quota-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.6rem 1rem;
    border-radius: var(--radius-md);
    font-size: 0.875rem;
    font-weight: 500;
  }

  .banner-body {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .banner-icon {
    display: inline-flex;
    flex-shrink: 0;
  }

  .banner-quota_warning {
    background: rgba(217, 119, 6, 0.08);
    color: var(--color-warning-text);
    border: 1px solid rgba(217, 119, 6, 0.24);
  }

  .banner-quota_exceeded {
    background: var(--danger-soft);
    color: var(--danger);
    border: 1px solid rgba(179, 36, 0, 0.28);
  }

  .banner-subscription_updated {
    background: var(--success-soft);
    color: var(--success);
    border: 1px solid rgba(26, 127, 75, 0.28);
  }

  .banner-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .btn-upgrade {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.3rem 0.7rem;
    background:    var(--color-accent);
    color:         var(--color-on-accent);
    border-radius: var(--radius-md);
    text-decoration: none;
    font-size:     var(--font-size-label);
    font-weight:   600;
    transition:
      transform  var(--duration-fast) var(--ease-standard),  /* [feedback] */
      box-shadow var(--duration-fast) var(--ease-standard);
    box-shadow: 0 2px 8px color-mix(in srgb, var(--color-accent) 25%, transparent);
  }

  .btn-upgrade:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px var(--color-accent-border, var(--color-border));
  }

  .btn-dismiss {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    cursor: pointer;
    color: inherit;
    opacity: 0.5;
    padding: 0.2rem;
    border-radius: var(--radius-xs);
    transition: opacity var(--duration-fast) var(--ease-standard);  /* [feedback] */
    min-width: var(--chip-h-sm);
    min-height: var(--chip-h-sm);
  }

  .btn-dismiss:hover { opacity: 1; }

  .btn-dismiss:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--color-accent-border, var(--color-border));
  }
</style>
