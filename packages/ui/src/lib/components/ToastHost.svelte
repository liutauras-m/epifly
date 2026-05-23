<svelte:options runes={true} />
<script lang="ts">
  /**
   * ToastHost — renders the global `toasts` singleton (Phase 4.10 audit).
   *
   * Positioning:
   *   Desktop (≥ 768px): top-right corner, below the topbar, inset var(--space-5)
   *   Mobile  (< 768px): top-center under the topbar, full-width minus gutters,
   *                       safe-area-inset-top aware so Dynamic Island is never covered
   *
   * Reads directly from the `toasts` module-level store so layout.svelte
   * can use `<ToastHost />` with no props and get automatic reactivity.
   */
  import { toasts } from '../stores/toast.svelte.js';
  import type { ToastKind } from '../stores/toast.svelte.js';
  import { X } from 'lucide-svelte';

  // Re-export Toast type for consumers
  export type Toast = { id: string; kind: ToastKind; message: string; };
</script>

{#if toasts.items.length > 0}
<div class="toast-host" aria-live="polite" aria-atomic="false">
  {#each toasts.items as toast (toast.id)}
    <div class="toast {toast.kind}" role="status" data-testid="toast">
      <span class="message">{toast.message}</span>
      <button
        class="dismiss"
        aria-label="Dismiss notification"
        onclick={() => toasts.dismiss(toast.id)}
      >
        <X size={16} strokeWidth={1.75} aria-hidden="true" />
      </button>
    </div>
  {/each}
</div>
{/if}

<style>
  /* ── Host positioning ────────────────────────────────────────────────────── */
  .toast-host {
    position: fixed;
    /* Mobile: top-center under the topbar, safe-area aware */
    top:    calc(var(--safe-top, 0px) + 56px + var(--space-3));
    left:   var(--space-3);
    right:  var(--space-3);
    display: flex;
    flex-direction: column;
    gap:    var(--space-2);
    z-index: 1000;
    pointer-events: none;
    align-items: center;
  }

  /* Medium+: top-right corner — container query so it works inside Tauri windows */
  @container app-shell (min-width: 768px) {
    .toast-host {
      top:        var(--space-5);
      left:       auto;
      right:      var(--space-5);
      max-width:  380px;
      align-items: flex-end;
    }
  }

  /* ── Toast ───────────────────────────────────────────────────────────────── */
  .toast {
    display:     flex;
    align-items: center;
    gap:         var(--space-3);
    padding:     var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    border:      1px solid var(--color-border);
    background:  var(--color-bg-raised);
    color:       var(--color-fg);
    font-size:   var(--font-size-meta);
    font-family: var(--font-family-sans);
    box-shadow:  0 4px 16px var(--color-shadow-md);
    pointer-events: auto;
    width:       100%;
    max-width:   380px;

    animation: toast-enter var(--duration-normal) var(--ease-emphasized-decelerate) both;
  }

  .toast.success {
    border-color: var(--color-success);
    background:   var(--color-success-soft);
    color:        var(--color-success);
  }
  .toast.error {
    border-color: var(--color-danger);
    background:   var(--color-danger-soft);
    color:        var(--color-danger);
  }
  .toast.warning {
    border-color: rgba(217, 119, 6, 0.4);
    background:   rgba(217, 119, 6, 0.10);
    color:        #b45309;
  }

  /* ── Parts ───────────────────────────────────────────────────────────────── */
  .message {
    flex:        1;
    color:       var(--color-fg);
    line-height: 1.4;
  }

  .toast.success .message,
  .toast.error   .message,
  .toast.warning .message {
    color: inherit;
  }

  .dismiss {
    display:         inline-flex;
    align-items:     center;
    justify-content: center;
    width:           28px;
    height:          28px;
    border:          none;
    background:      transparent;
    color:           currentColor;
    cursor:          pointer;
    border-radius:   var(--radius-sm);
    padding:         0;
    flex-shrink:     0;
    opacity:         0.6;
    transition:      opacity var(--duration-fast), background var(--duration-fast);
    outline:         none;
  }
  .dismiss:hover { opacity: 1; background: rgba(0,0,0,0.06); }
  .dismiss:focus-visible {
    outline:        var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }

  /* ── Animation ───────────────────────────────────────────────────────────── */
  @keyframes toast-enter {
    from {
      transform: translateY(-12px) scale(0.97);
      opacity:   0;
    }
    to {
      transform: translateY(0) scale(1);
      opacity:   1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .toast { animation: none; }
  }
</style>
