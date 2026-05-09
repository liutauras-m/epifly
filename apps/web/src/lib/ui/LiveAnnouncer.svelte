<script lang="ts">
  import { toasts } from './toast.svelte';
</script>

<!--
  Renders toast messages and provides a visually-hidden aria-live region
  so screen readers announce them automatically.
-->
<div class="announcer" aria-live="polite" aria-atomic="false">
  {#each toasts.items as toast (toast.id)}
    <span>{toast.message}</span>
  {/each}
</div>

{#if toasts.items.length > 0}
  <div class="toast-stack" role="status" aria-label="Notifications">
    {#each toasts.items as toast (toast.id)}
      <div class="toast toast-{toast.kind}">
        <span class="toast-msg">{toast.message}</span>
        <button class="toast-close" aria-label="Dismiss" onclick={() => toasts.dismiss(toast.id)}>
          <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="12" height="12">
            <line x1="2" y1="2" x2="10" y2="10"/><line x1="10" y1="2" x2="2" y2="10"/>
          </svg>
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .announcer {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0,0,0,0);
    white-space: nowrap;
    border: 0;
  }

  .toast-stack {
    position: fixed;
    bottom: 1.5rem;
    right: 1.5rem;
    z-index: 9999;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: 22rem;
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.625rem 0.875rem;
    border-radius: 0.375rem;
    font-size: 0.875rem;
    line-height: 1.4;
    box-shadow: 0 2px 8px rgba(0,0,0,0.12);
    background: var(--surface, #fff);
    border-left: 3px solid var(--accent, #0d9488);
    animation: toast-in 160ms ease;
  }

  .toast-success { border-left-color: #16a34a; }
  .toast-error { border-left-color: #dc2626; }
  .toast-warning { border-left-color: #d97706; }

  .toast-msg { flex: 1; }

  .toast-close {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    opacity: 0.5;
    flex-shrink: 0;
    margin-top: 1px;
  }
  .toast-close:hover { opacity: 1; }

  @keyframes toast-in {
    from { opacity: 0; transform: translateY(0.5rem); }
    to   { opacity: 1; transform: translateY(0); }
  }
</style>
