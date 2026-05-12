<script lang="ts">
  export let label: string;
  export let used: number = 0;
  export let limit: number | null = null;
  export let unit: string = '';
  export let warnAt: number = 0.8;

  $: pct = limit !== null && limit > 0 ? Math.min(used / limit, 1) : 0;
  $: hasLimit = limit !== null;
  $: isWarn = pct >= warnAt && pct < 1;
  $: isExceeded = pct >= 1;

  function fmt(n: number): string {
    return n.toLocaleString();
  }
</script>

<div class="usage-meter">
  <div class="meter-header">
    <span class="meter-label">{label}</span>
    <span class="meter-value" class:warn={isWarn} class:exceeded={isExceeded}>
      {fmt(used)}{unit}
      {#if hasLimit} / {fmt(limit!)}{unit}{/if}
    </span>
  </div>

  {#if hasLimit}
    <div class="bar-track" role="progressbar" aria-valuenow={used} aria-valuemax={limit ?? 0} aria-label="{label} usage">
      <div
        class="bar-fill"
        class:warn={isWarn}
        class:exceeded={isExceeded}
        style="width: {(pct * 100).toFixed(1)}%"
      ></div>
    </div>
    <div class="meter-footer">
      {#if isExceeded}
        <span class="exceeded">Limit reached</span>
      {:else}
        <span class="remaining">{fmt((limit ?? 0) - used)}{unit} remaining</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .usage-meter { display: flex; flex-direction: column; gap: 0.35rem; }
  .meter-header { display: flex; justify-content: space-between; align-items: baseline; }
  .meter-label { font-weight: 600; font-size: 0.875rem; }
  .meter-value { font-size: 0.875rem; color: #374151; }
  .meter-value.warn { color: #d97706; }
  .meter-value.exceeded { color: #dc2626; font-weight: 600; }
  .bar-track {
    height: 6px; background: #e5e7eb; border-radius: 999px; overflow: hidden;
  }
  .bar-fill {
    height: 100%; background: #6366f1; border-radius: 999px;
    transition: width 0.3s ease;
  }
  .bar-fill.warn { background: #f59e0b; }
  .bar-fill.exceeded { background: #ef4444; }
  .meter-footer { font-size: 0.75rem; color: #6b7280; }
  .remaining { color: #6b7280; }
  .exceeded { color: #dc2626; font-weight: 600; }
</style>
