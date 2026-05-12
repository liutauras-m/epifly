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
    font-family: var(--font-body);
    font-weight: 600;
    font-size: 0.875rem;
    color: var(--ink);
  }

  .meter-value {
    font-family: var(--font-display);
    font-size: 0.875rem;
    font-weight: 600;
    letter-spacing: -0.02em;
    color: var(--ink-2);
    transition: color 180ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  .meter-value.warn    { color: #d97706; }   /* amber-600 — warning, not brand orange */
  .meter-value.exceeded { color: var(--danger); font-weight: 700; }

  .bar-track {
    height: 6px;
    background: var(--paper-3);
    border-radius: var(--r-full);
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    background: var(--ember);
    border-radius: var(--r-full);
    transition: width 300ms cubic-bezier(0.4, 0, 0.2, 1),
                background 180ms cubic-bezier(0.4, 0, 0.2, 1);
  }

  .bar-fill.warn     { background: #d97706; }
  .bar-fill.exceeded { background: var(--danger); }

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
