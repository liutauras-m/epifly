<script lang="ts">
  import { useCapabilityRendererRegistry } from '../capabilities/CapabilityRendererRegistry.svelte.js';
  import type { CapabilityCard } from '@conusai/types';

  let {
    id,
    name,
    status,
    result,
    startTime,
    capabilityCard = undefined,
    onRetry = undefined,
  }: {
    id: string;
    name: string;
    status: 'running' | 'success' | 'error';
    result: string;
    startTime: number;
    capabilityCard?: CapabilityCard;
    onRetry?: () => void;
  } = $props();

  const registry = useCapabilityRendererRegistry();
  const Renderer = $derived(capabilityCard ? registry.get(capabilityCard) : null);

  // Capture elapsed once when status leaves 'running' so the time doesn't drift.
  let elapsedMs = $state<number | null>(null);
  $effect(() => {
    if (status !== 'running' && elapsedMs === null) {
      elapsedMs = Math.round(performance.now() - startTime);
    }
  });

  // Format compound tool name (e.g. "media_time__get_current_time" → "get_current_time")
  // and replace underscores with spaces for readability.
  const displayName = $derived(
    (name.includes('__') ? name.split('__').pop()! : name).replaceAll('_', ' ')
  );
</script>

{#if Renderer && capabilityCard}
  <Renderer card={capabilityCard} />
{:else}
  <details class="tool-card" data-status={status}>
    <summary class="tool-head">
      <span class="tool-dot" role="status" aria-label={status}></span>
      <span class="tool-name">{displayName}</span>
      <span class="tool-time" aria-label={status === 'running' ? 'Running' : `${elapsedMs}ms`}>
        {#if status !== 'running'}{elapsedMs}ms{:else}…{/if}
      </span>
      {#if status === 'error' && onRetry}
        <button class="retry-btn" onclick={(e) => { e.preventDefault(); onRetry!(); }} aria-label="Retry tool call">
          Retry
        </button>
      {/if}
    </summary>
    <div class="tool-body">{result || 'running…'}</div>
  </details>
{/if}

<style>
  .tool-card {
    border: 1px solid var(--rule);
    border-radius: var(--radius-sm);
    margin: var(--space-2) 0;
    font-size: var(--font-size-meta);
    overflow: hidden;
  }
  .tool-head {
    display: flex; align-items: center; gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    cursor: pointer; list-style: none;
    background: var(--paper-2);
  }
  .tool-dot {
    width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0;
    background: var(--ink-3);
  }
  [data-status="running"] .tool-dot { background: var(--ember); animation: pulse 1.2s infinite; }
  [data-status="success"] .tool-dot { background: var(--success); }
  [data-status="error"]   .tool-dot { background: var(--danger); }
  .tool-name { flex: 1; font-family: var(--font-mono); }
  .tool-time { color: var(--ink-3); }
  .retry-btn {
    margin-left: var(--space-2);
    padding: 1px var(--space-2);
    font-size: var(--font-size-meta);
    border: 1px solid var(--danger);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--danger);
    cursor: pointer;
    flex-shrink: 0;
  }
  .retry-btn:hover { background: color-mix(in srgb, var(--danger) 12%, transparent); }
  .tool-body {
    padding: var(--space-3);
    font-family: var(--font-mono);
    font-size: var(--font-size-mono);
    white-space: pre-wrap;
    word-break: break-all;
    background: var(--paper);
    max-height: 200px;
    overflow: auto;
  }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.3} }
</style>
