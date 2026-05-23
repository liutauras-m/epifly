<svelte:options runes={true} />
<script lang="ts">
  /**
   * StatusBadge — generic status indicator (Phase 4.5).
   *
   * Pure semantic component: maps a status to a color-coded pill.
   * For billing-specific labeling see InvoiceStatusBadge in features/billing/.
   *
   * Usage:
   *   <StatusBadge status="success" label="Paid" />
   *   <StatusBadge status="danger" label="Overdue" />
   */
  export type StatusKind = 'success' | 'warning' | 'danger' | 'neutral' | 'info';

  let {
    status,
    label,
    class: cls = '',
  }: {
    status:  StatusKind;
    label:   string;
    class?:  string;
  } = $props();
</script>

<span class="status-badge status-{status}{cls ? ` ${cls}` : ''}">
  <span class="status-dot" aria-hidden="true"></span>
  {label}
</span>

<style>
  .status-badge {
    display:        inline-flex;
    align-items:    center;
    gap:            5px;
    padding:        2px var(--space-2);
    border-radius:  var(--radius-full);
    font-family:    var(--font-family-mono);
    font-size:      var(--font-size-label);   /* 11px */
    font-weight:    600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    white-space:    nowrap;
    border:         1px solid transparent;
  }

  /* Dot indicator */
  .status-dot {
    width:         6px;
    height:        6px;
    border-radius: var(--radius-full);
    background:    currentColor;
    flex-shrink:   0;
  }

  /* ── Status color map ─────────────────────────────────────────────────────── */
  .status-success {
    background: var(--color-success-soft);
    color:      var(--color-success);
    border-color: var(--color-success-soft);
  }

  .status-warning {
    background: rgba(217, 119, 6, 0.12);
    color:      #d97706;
    border-color: rgba(217, 119, 6, 0.24);
  }

  .status-danger {
    background: var(--color-danger-soft);
    color:      var(--color-danger);
    border-color: var(--color-danger-soft);
  }

  .status-neutral {
    background: var(--color-bg-hover);
    color:      var(--color-fg-subtle);
    border-color: var(--color-border);
  }

  .status-info {
    background: var(--cyan-soft, rgba(0,212,255,0.10));
    color:      var(--cyan, #00D4FF);
    border-color: var(--cyan-soft, rgba(0,212,255,0.10));
  }
</style>
