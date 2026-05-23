<script lang="ts">
  /**
   * ToastHost — renders the global `toasts` singleton.
   *
   * Reads directly from the `toasts` module-level store so layout.svelte
   * can use `<ToastHost />` with no props and get automatic reactivity.
   */
  import { toasts } from '../stores/toast.svelte.js';
  import { X } from 'lucide-svelte';
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
  .toast-host {
    position: fixed;
    /* Mobile: full-width strip above the keyboard / home indicator */
    bottom: calc(var(--space-4) + env(safe-area-inset-bottom, 0px));
    left: var(--space-3);
    right: var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    z-index: 1000;
    pointer-events: none;
  }

  /* Desktop: compact right-aligned stack */
  @media (min-width: 641px) {
    .toast-host {
      left: auto;
      right: var(--space-5);
      bottom: var(--space-5);
      max-width: 360px;
    }
  }

  .toast {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-sm);
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink);
    font-size: var(--font-size-meta, 13px);
    box-shadow: var(--shadow);
    animation: slide-up var(--duration-normal, 200ms) var(--ease-out, cubic-bezier(0.4, 0, 0.2, 1));
    pointer-events: auto;
  }

  .toast.success { border-color: var(--success); background: var(--success-soft); }
  .toast.error   { border-color: var(--danger);  background: var(--danger-soft); }
  .toast.warning { border-color: var(--warning, #d97706); background: var(--warning-soft, #fffbeb); }

  .message { flex: 1; }

  .dismiss {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    background: transparent;
    color: var(--ink-3);
    cursor: pointer;
    border-radius: var(--radius-sm);
    padding: 0;
    flex-shrink: 0;
    transition: background var(--duration-fast, 100ms);
  }
  .dismiss:hover {
    background: var(--paper-3);
    color: var(--ink);
  }

  @keyframes slide-up {
    from { transform: translateY(16px); opacity: 0; }
    to   { transform: translateY(0);    opacity: 1; }
  }

  @media (prefers-reduced-motion: reduce) {
    .toast { animation: none; }
  }
</style>
