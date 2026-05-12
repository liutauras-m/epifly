<script lang="ts">
  export let tier: string = 'free';
  export let status: string = 'active';

  const colors: Record<string, string> = {
    free: 'badge-free',
    pro: 'badge-pro',
    team: 'badge-team',
    enterprise: 'badge-enterprise',
  };

  const icons: Record<string, string> = {
    free: '🆓',
    pro: '⚡',
    team: '👥',
    enterprise: '🏢',
  };

  $: colorClass = colors[tier.toLowerCase()] ?? 'badge-free';
  $: icon = icons[tier.toLowerCase()] ?? '📦';
  $: isDegraded = status === 'past_due' || status === 'canceled';
</script>

<span class="plan-badge {colorClass}" class:degraded={isDegraded} title="Plan: {tier} ({status})">
  {icon} {tier.toUpperCase()}
  {#if isDegraded}
    <span class="status-dot" aria-label="subscription {status}">⚠</span>
  {/if}
</span>

<style>
  .plan-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    padding: 0.15rem 0.6rem;
    border-radius: 999px;
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    white-space: nowrap;
  }
  .badge-free       { background: #f3f4f6; color: #6b7280; }
  .badge-pro        { background: #ede9fe; color: #7c3aed; }
  .badge-team       { background: #dbeafe; color: #1d4ed8; }
  .badge-enterprise { background: #fef3c7; color: #92400e; }
  .degraded         { opacity: 0.7; }
  .status-dot       { font-size: 0.65rem; }
</style>
