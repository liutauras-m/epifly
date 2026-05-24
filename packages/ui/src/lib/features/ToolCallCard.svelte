<script lang="ts">
  /**
   * ToolCallCard — feature wrapper for <ToolCard> (Phase 4.2).
   *
   * Routes to a capability-specific Renderer when one is registered for this
   * tool's CapabilityCard; falls back to the generic <ToolCard> primitive.
   *
   * The pure primitive is packages/ui/src/lib/components/ToolCard.svelte.
   */
  import { useCapabilityRendererRegistry } from '../capabilities/CapabilityRendererRegistry.svelte.js';
  import type { CapabilityCard } from '@conusai/types';
  import ToolCard from '../components/ToolCard.svelte';

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
</script>

{#if Renderer && capabilityCard}
  <Renderer card={capabilityCard} />
{:else}
  <ToolCard {id} {name} {status} {result} {startTime} {onRetry} />
{/if}
