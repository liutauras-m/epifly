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
  }: {
    id: string;
    name: string;
    status: 'running' | 'success' | 'error';
    result: string;
    startTime: number;
    capabilityCard?: CapabilityCard;
  } = $props();

  const registry = useCapabilityRendererRegistry();
  const Renderer = $derived(capabilityCard ? registry.get(capabilityCard) : null);
</script>

{#if Renderer && capabilityCard}
  <Renderer card={capabilityCard} />
{:else}
  <details class="tool-card" data-status={status}>
    <summary class="tool-head">
      <span class="tool-dot" role="status" aria-label={status}></span>
      <span class="tool-name">{name}</span>
      <span class="tool-time" aria-label={status === 'running' ? 'Running' : `${Math.round(performance.now() - startTime)}ms`}>
        {#if status !== 'running'}{Math.round(performance.now() - startTime)}ms{:else}…{/if}
      </span>
    </summary>
    <div class="tool-body">{result || 'running…'}</div>
  </details>
{/if}

<style>
  .tool-card {
    border: 1px solid var(--rule);
    border-radius: var(--r-sm);
    margin: var(--s-2) 0;
    font-size: var(--t-meta);
    overflow: hidden;
  }
  .tool-head {
    display: flex; align-items: center; gap: var(--s-2);
    padding: var(--s-2) var(--s-3);
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
  .tool-body {
    padding: var(--s-3);
    font-family: var(--font-mono);
    font-size: var(--t-mono);
    white-space: pre-wrap;
    word-break: break-all;
    background: var(--paper);
    max-height: 200px;
    overflow: auto;
  }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.3} }
</style>
