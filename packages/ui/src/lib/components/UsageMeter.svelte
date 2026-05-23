<svelte:options runes={true} />
<script lang="ts">
  let {
    label,
    used = 0,
    limit = null,
    unit = '',
    warnAt = 0.8,
  }: {
    label: string;
    used?: number;
    limit?: number | null;
    unit?: string;
    warnAt?: number;
  } = $props();

  const pct       = $derived(limit !== null && limit > 0 ? Math.min(used / limit, 1) : 0);
  const hasLimit  = $derived(limit !== null);
  const isWarn    = $derived(pct >= warnAt && pct < 1);
  const isExceeded = $derived(pct >= 1);

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
    <div
      class="bar-track"
      role="progressbar"
      aria-valuenow={used}
      aria-valuemax={limit ?? 0}
      aria-label="{label} usage"
    >
      <div
        class="bar-fill"
        class:warn={isWarn}
        class:exceeded={isExceeded}
        style="width: {(pct * 100).toFixed(1)}%"
      ></div>
    </div>
    <div class="meter-footer">
      {#if isExceeded}
        <span class="status exceeded">Limit reached</span>
      {:else if isWarn}
        <span class="status warn">{fmt((limit ?? 0) - used)}{unit} remaining</span>
      {:else}
        <span class="status">{fmt((limit ?? 0) - used)}{unit} remaining</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .usage-meter {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .meter-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .meter-label {
    font-family: var(--font-family-sans);
    font-weight: 600;
    font-size:   var(--font-size-meta);
    color:       var(--color-fg);
  }

  .meter-value {
    font-family:    var(--font-family-sans);
    font-size:      var(--font-size-meta);
    font-weight:    600;
    letter-spacing: -0.02em;
    color:          var(--color-fg-muted);
    transition:     color var(--duration-fast) var(--ease-standard);  /* [feedback] */
  }

  .meter-value.warn     { color: var(--color-warning, #d97706); }
  .meter-value.exceeded { color: var(--color-danger);  font-weight: 700; }

  .bar-track {
    height:        6px;
    background:    var(--color-bg-hover);
    border-radius: var(--radius-full);
    overflow:      hidden;
  }

  .bar-fill {
    height:        100%;
    background:    var(--color-accent);
    border-radius: var(--radius-full);
    transition:
      width      var(--duration-normal) var(--ease-standard),  /* [feedback] */
      background var(--duration-fast)   var(--ease-standard);
  }

  .bar-fill.warn     { background: var(--color-warning, #d97706); }
  .bar-fill.exceeded { background: var(--color-danger); }

  .meter-footer {
    display: flex;
  }

  .status {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.04em;
    color: var(--ink-3);
  }

  .status.warn     { color: #d97706; }
  .status.exceeded { color: var(--danger); font-weight: 600; }
</style>
