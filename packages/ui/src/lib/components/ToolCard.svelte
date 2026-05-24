<svelte:options runes={true} />
<script lang="ts">
  /**
   * ToolCard — expandable tool call status card (Phase 4.2).
   *
   * A pure primitive: props in, callbacks out. Renders a <details>-based
   * collapsible with a status dot, tool name, elapsed time, and result body.
   *
   * The feature wrapper <ToolCallCard> in features/ adds capability-renderer
   * routing on top of this primitive.
   *
   * Usage:
   *   <ToolCard id="t1" name="web_search" status="success" result={output} startTime={t0} />
   */
  import { t } from '../utils/i18n.js';

  let {
    id,
    name,
    status,
    result,
    startTime,
    onRetry,
  }: {
    id: string;
    name: string;
    status: 'running' | 'success' | 'error';
    result: string;
    startTime: number;
    onRetry?: () => void;
  } = $props();

  // Capture elapsed once when status leaves 'running' so the time doesn't drift.
  let elapsedMs = $state<number | null>(null);
  $effect(() => {
    if (status !== 'running' && elapsedMs === null) {
      elapsedMs = Math.round(performance.now() - startTime);
    }
  });

  // Format compound tool name ("media_time__get_current_time" → "get current time")
  const displayName = $derived(
    (name.includes('__') ? name.split('__').pop()! : name).replaceAll('_', ' ')
  );
</script>

<!-- role="status" announces state transitions to screen readers -->
<details
  class="tool-card"
  data-status={status}
  role="status"
  aria-label="{displayName}: {status}"
  {id}
>
  <summary class="tool-head">
    <span class="tool-dot" aria-label={status}></span>
    <span class="tool-name">{displayName}</span>
    <span
      class="tool-time"
      aria-label={status === 'running' ? t('tool.running') : `${elapsedMs}ms`}
    >
      {#if status !== 'running'}{elapsedMs}ms{:else}…{/if}
    </span>
    {#if status === 'error' && onRetry}
      <button
        class="retry-btn"
        onclick={(e) => { e.preventDefault(); onRetry!(); }}
        aria-label={t('tool.retry')}
      >
        {t('tool.retry')}
      </button>
    {/if}
  </summary>
  <div class="tool-body">{result || 'running…'}</div>
</details>

<style>
  .tool-card {
    border:        1px solid var(--color-border);
    border-radius: var(--radius-sm);
    margin:        var(--space-2) 0;
    font-size:     var(--font-size-meta);
    overflow:      hidden;
  }

  .tool-head {
    display:     flex;
    align-items: center;
    gap:         var(--space-2);
    padding:     var(--space-2) var(--space-3);
    cursor:      pointer;
    list-style:  none;
    background:  var(--color-bg-raised);
  }
  /* Remove default marker on Chrome */
  .tool-head::-webkit-details-marker { display: none; }

  .tool-dot {
    width:         var(--_dot-size, 8px);
    height:        var(--_dot-size, 8px);
    border-radius: 50%;
    flex-shrink:   0;
    background:    var(--color-fg-subtle);
  }
  [data-status="running"] .tool-dot {
    background: var(--color-accent);
    animation:  pulse 1.2s infinite;  /* [feedback] tool actively running */
  }
  [data-status="success"] .tool-dot { background: var(--color-success, var(--color-accent)); }
  [data-status="error"]   .tool-dot { background: var(--color-danger); }

  /* [feedback] running→success/error radial flash — result acknowledgement (~280ms) */
  [data-status="success"] .tool-head {
    animation: card-flash-success 280ms var(--ease-emphasized-decelerate, cubic-bezier(0.05, 0.7, 0.1, 1)) both;  /* [feedback] */
  }
  [data-status="error"] .tool-head {
    animation: card-flash-error 280ms var(--ease-emphasized-decelerate, cubic-bezier(0.05, 0.7, 0.1, 1)) both;  /* [feedback] */
  }
  @keyframes card-flash-success {
    0%   { background: var(--color-success-soft, #f0fdf4); }
    100% { background: transparent; }
  }
  @keyframes card-flash-error {
    0%   { background: var(--color-danger-soft, #fef2f2); }
    100% { background: transparent; }
  }

  .tool-name {
    flex:        1;
    font-family: var(--font-family-mono);
  }
  .tool-time { color: var(--color-fg-subtle); }

  .retry-btn {
    margin-left:   var(--space-2);
    padding:       var(--_btn-pv, 1px) var(--space-2);
    font-size:     var(--font-size-meta);
    border:        1px solid var(--color-danger);
    border-radius: var(--radius-sm);
    background:    transparent;
    color:         var(--color-danger);
    cursor:        pointer;
    flex-shrink:   0;
  }
  .retry-btn:hover {
    background: color-mix(in srgb, var(--color-danger) 12%, transparent);
  }

  .tool-body {
    padding:     var(--space-3);
    font-family: var(--font-family-mono);
    font-size:   var(--font-size-meta);
    white-space: pre-wrap;
    word-break:  break-all;
    background:  var(--color-bg);
    max-height:  var(--_result-max-h, 200px);
    overflow:    auto;
  }

  @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }

  @media (prefers-reduced-motion: reduce) {
    [data-status="running"] .tool-dot { animation: none; }
  }
</style>
