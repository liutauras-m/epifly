<script lang="ts">
  import { onDestroy } from 'svelte';

  export let apiBase: string = '';
  export let upgradeUrl: string = '/account/billing';

  type BannerKind = 'quota_warning' | 'quota_exceeded' | 'subscription_updated';

  interface BannerEvent {
    kind: BannerKind;
    message?: string;
    plan?: string;
  }

  let banner: BannerEvent | null = null;
  let es: EventSource | null = null;

  function connect() {
    if (typeof EventSource === 'undefined') return;
    es = new EventSource(`${apiBase}/v1/realtime`);

    es.addEventListener('quota.warning', (e: MessageEvent) => {
      banner = { kind: 'quota_warning', message: 'You are approaching your daily quota.' };
    });

    es.addEventListener('quota.exceeded', (e: MessageEvent) => {
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
  <div class="quota-banner banner-{banner.kind}" role="alert">
    <span class="banner-msg">
      {#if banner.kind === 'quota_exceeded'}⛔{:else if banner.kind === 'quota_warning'}⚠️{:else}✅{/if}
      {banner.message ?? ''}
    </span>
    <div class="banner-actions">
      {#if banner.kind !== 'subscription_updated'}
        <a href={upgradeUrl} class="btn-upgrade">Upgrade Plan</a>
      {/if}
      <button class="btn-dismiss" on:click={dismiss} aria-label="Dismiss">✕</button>
    </div>
  </div>
{/if}

<style>
  .quota-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.65rem 1rem;
    border-radius: 8px;
    font-size: 0.875rem;
    font-weight: 500;
  }
  .banner-quota_warning  { background: #fffbeb; color: #92400e; border: 1px solid #fde68a; }
  .banner-quota_exceeded { background: #fee2e2; color: #991b1b; border: 1px solid #fecaca; }
  .banner-subscription_updated { background: #d1fae5; color: #065f46; border: 1px solid #6ee7b7; }
  .banner-actions { display: flex; align-items: center; gap: 0.5rem; flex-shrink: 0; }
  .btn-upgrade {
    padding: 0.3rem 0.75rem;
    background: #6366f1; color: #fff;
    border-radius: 6px; text-decoration: none;
    font-size: 0.8rem; font-weight: 600;
  }
  .btn-upgrade:hover { background: #4f46e5; }
  .btn-dismiss {
    background: none; border: none; cursor: pointer;
    font-size: 0.875rem; color: inherit; opacity: 0.6; padding: 0.1rem 0.3rem;
  }
  .btn-dismiss:hover { opacity: 1; }
</style>
