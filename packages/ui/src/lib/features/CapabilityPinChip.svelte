<script lang="ts">
  /**
   * Clickable routing-audit chip (PR 3.B.1 / 3.B.1.1).
   *
   * Renders whenever the gateway emits a `routing_meta` SSE delta — i.e. on every
   * turn. The chip shows the pinned capability name (or "Routing" when natural-
   * language routing chose the tools). Click expands a popover with the full
   * audit: forced_capability, selected_capabilities, pinned_tools, lexical_hits,
   * max_score — so users can see *exactly* which tools the model had this turn.
   *
   * Platform split:
   *   - Desktop ((min-width: 721px)): non-modal `<dialog>` positioned beneath
   *     the chip. `Esc` is native dialog behaviour; backdrop click via `cancel`.
   *   - Mobile: `AppBottomSheet` (existing primitive, scroll-lock-aware).
   *
   * Cross-app parity: this component lives in `packages/ui/src/lib/features` and
   * is consumed identically by `apps/web` and `apps/browser-shell`.
   */
  import type { RoutingMeta } from '@conusai/sdk';
  import { Pin, Info, X } from '@lucide/svelte';
  import Sheet from '../components/Sheet.svelte';

  let {
    routingMeta,
    onDismiss = undefined,
  }: {
    routingMeta: RoutingMeta;
    /** Optional close-out callback shown as a tiny ✕ on the chip. */
    onDismiss?: () => void;
  } = $props();

  // Derive a human-readable label from the capability name.
  // e.g. "code-project" → "Code Project", "media_time" → "Media Time"
  const capLabel = $derived(
    routingMeta.forced_capability
      ? routingMeta.forced_capability
          .replace(/[-_]/g, ' ')
          .replace(/\b\w/g, (c: string) => c.toUpperCase())
      : 'Routing'
  );

  let chipEl = $state<HTMLButtonElement | undefined>();
  let popoverEl = $state<HTMLDialogElement | undefined>();
  let popoverOpen = $state(false);
  let popoverPos = $state<{ top: number; left: number } | null>(null);

  // Mobile breakpoint detection — re-evaluated on resize via a $effect.
  let isMobile = $state(false);
  $effect(() => {
    function recompute() {
      isMobile = typeof window !== 'undefined' && window.matchMedia('(max-width: 720px)').matches;
    }
    recompute();
    if (typeof window === 'undefined') return;
    window.addEventListener('resize', recompute);
    return () => window.removeEventListener('resize', recompute);
  });

  function openPopover() {
    if (isMobile) {
      popoverOpen = true;
      return;
    }
    if (!chipEl) return;
    const rect = chipEl.getBoundingClientRect();
    popoverPos = {
      top: rect.bottom + window.scrollY + 6,
      left: rect.left + window.scrollX,
    };
    popoverOpen = true;
    // Use rAF so the dialog is in the DOM before we call show()/showModal().
    requestAnimationFrame(() => popoverEl?.show?.());
  }

  function closePopover() {
    popoverOpen = false;
    if (!isMobile) popoverEl?.close?.();
  }

  function commaList(items: string[]): string {
    return items.length > 0 ? items.join(', ') : '(none)';
  }

  function fmt2(n: number): string {
    return n.toFixed(2);
  }
</script>

<button
  bind:this={chipEl}
  type="button"
  class="pin-chip"
  aria-haspopup="dialog"
  aria-expanded={popoverOpen}
  aria-label="Routing details: {capLabel}"
  onclick={openPopover}
>
  {#if routingMeta.forced_capability}
    <Pin size={14} strokeWidth={1.75} aria-hidden="true" />
  {:else}
    <Info size={14} strokeWidth={1.75} aria-hidden="true" />
  {/if}
  <span class="pin-label">{capLabel}</span>
  {#if onDismiss}
    <span
      role="button"
      tabindex="0"
      class="dismiss-btn"
      aria-label="Dismiss capability pin"
      onclick={(e: MouseEvent) => { e.stopPropagation(); onDismiss?.(); }}
      onkeydown={(e: KeyboardEvent) => {
        if (e.key === 'Enter' || e.key === ' ') { e.stopPropagation(); onDismiss?.(); }
      }}
    ><X size={12} strokeWidth={1.75} aria-hidden="true" /></span>
  {/if}
</button>

{#if popoverOpen && isMobile}
  <Sheet open={true} onclose={closePopover} label="Routing details">
    {#snippet children()}
      <div class="popover-content">
        <h3 class="popover-title">Routing details</h3>
        <dl class="popover-list">
          <dt>Forced capability</dt>
          <dd>{routingMeta.forced_capability ?? '(none)'}</dd>
          <dt>Selected capabilities</dt>
          <dd>{commaList(routingMeta.selected_capabilities)}</dd>
          <dt>Pinned tools</dt>
          <dd class="mono">{commaList(routingMeta.pinned_tools)}</dd>
          <dt>Lexical hits</dt>
          <dd>{commaList(routingMeta.lexical_hits)}</dd>
          <dt>Max score</dt>
          <dd class="mono">{fmt2(routingMeta.max_score)}</dd>
        </dl>
      </div>
    {/snippet}
  </Sheet>
{:else if popoverOpen && popoverPos}
  <dialog
    bind:this={popoverEl}
    class="popover-dialog"
    style="position: absolute; top: {popoverPos.top}px; left: {popoverPos.left}px;"
    oncancel={closePopover}
    onkeydown={(e: KeyboardEvent) => { if (e.key === 'Escape') closePopover(); }}
  >
    <div class="popover-content">
      <header class="popover-header">
        <h3 class="popover-title">Routing details</h3>
        <button type="button" class="popover-close" aria-label="Close" onclick={closePopover}><X size={16} strokeWidth={1.75} aria-hidden="true" /></button>
      </header>
      <dl class="popover-list">
        <dt>Forced capability</dt>
        <dd>{routingMeta.forced_capability ?? '(none)'}</dd>
        <dt>Selected capabilities</dt>
        <dd>{commaList(routingMeta.selected_capabilities)}</dd>
        <dt>Pinned tools</dt>
        <dd class="mono">{commaList(routingMeta.pinned_tools)}</dd>
        <dt>Lexical hits</dt>
        <dd>{commaList(routingMeta.lexical_hits)}</dd>
        <dt>Max score</dt>
        <dd class="mono">{fmt2(routingMeta.max_score)}</dd>
      </dl>
    </div>
  </dialog>
{/if}

<style>
  .pin-chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 2px var(--space-2);
    border-radius: var(--radius-full, 9999px);
    border: 1px solid color-mix(in srgb, var(--accent, var(--color-accent)) 40%, transparent);
    background: color-mix(in srgb, var(--accent, var(--color-accent)) 10%, transparent);
    color: var(--accent, var(--color-accent));
    font-size: var(--font-size-meta);
    font-family: var(--font-sans, var(--font-family-sans));
    line-height: 1.4;
    cursor: pointer;
    user-select: none;
    transition: filter var(--duration-fast); /* [feedback] */
  }
  .pin-chip:hover { filter: brightness(1.08); }
  .pin-chip:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }
  .pin-label { font-weight: 500; }
  .dismiss-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    padding: 0 2px;
    cursor: pointer;
    color: inherit;
    opacity: 0.6;
    line-height: 1;
  }
  .dismiss-btn:hover { opacity: 1; }

  .popover-dialog {
    z-index: 1000;
    margin: 0;
    padding: 0;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg);
    color: var(--color-fg);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.15);
    min-width: 280px;
    max-width: 360px;
  }
  .popover-dialog::backdrop { background: transparent; }

  .popover-content {
    padding: var(--space-3) var(--space-4);
  }
  .popover-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--color-border);
    margin-bottom: var(--space-2);
  }
  .popover-title {
    margin: 0;
    font-size: var(--font-size-meta);
    font-weight: 600;
    color: var(--color-fg);
  }
  .popover-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--color-fg-subtle);
    cursor: pointer;
    padding: 2px;
    border-radius: var(--radius-sm);
    line-height: 1;
  }
  .popover-close:hover { color: var(--color-fg); }

  .popover-list {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--space-1) var(--space-3);
    margin: 0;
    font-size: var(--font-size-meta);
  }
  .popover-list dt {
    color: var(--color-fg-subtle);
    font-weight: 500;
  }
  .popover-list dd {
    margin: 0;
    color: var(--color-fg);
    word-break: break-word;
  }
  .popover-list dd.mono {
    font-family: var(--font-family-mono);
    font-size: 0.92em;
  }
</style>
