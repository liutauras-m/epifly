<script lang="ts">
  export interface Toast {
    id: string;
    message: string;
    kind?: "info" | "success" | "error";
    durationMs?: number;
  }

  interface Props {
    toasts?: Toast[];
    ondismiss?: (id: string) => void;
  }

  let { toasts = [], ondismiss }: Props = $props();
</script>

<div class="toast-host" aria-live="polite" aria-atomic="false">
  {#each toasts as toast (toast.id)}
    <div class="toast {toast.kind ?? 'info'}" role="status">
      <span class="message">{toast.message}</span>
      <button
        class="dismiss"
        aria-label="Dismiss notification"
        onclick={() => ondismiss?.(toast.id)}
      >×</button>
    </div>
  {/each}
</div>

<style>
  .toast-host {
    position: fixed;
    bottom: var(--s-5);
    right: var(--s-5);
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
    z-index: 1000;
    max-width: 360px;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    padding: var(--s-3) var(--s-4);
    border-radius: 8px;
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink);
    font-size: 13px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.12);
    animation: slide-in var(--dur-2) var(--ease-out);
  }

  .toast.success { border-color: var(--success); background: var(--success-soft); }
  .toast.error   { border-color: var(--danger);  background: var(--danger-soft); }

  .message { flex: 1; }

  .dismiss {
    border: none;
    background: transparent;
    color: var(--ink-3);
    cursor: pointer;
    font-size: 18px;
    line-height: 1;
    padding: 0;
  }

  @keyframes slide-in {
    from { transform: translateX(100%); opacity: 0; }
    to   { transform: translateX(0);   opacity: 1; }
  }
</style>
