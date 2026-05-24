<svelte:options runes={true} />
<script lang="ts">
  /**
   * InvoiceStatusBadge — billing-aware wrapper over StatusBadge (Phase 4.5).
   *
   * Maps Stripe invoice statuses to the generic color semantics of StatusBadge.
   * Once billing adds retry URLs, payment-provider state, or dunning logic,
   * those belong here — not in the generic StatusBadge primitive.
   *
   * Usage:
   *   <InvoiceStatusBadge status="paid" />
   *   <InvoiceStatusBadge status="past_due" />
   */
  import StatusBadge, { type StatusKind } from '../../components/StatusBadge.svelte';

  export type InvoiceStatus = 'paid' | 'open' | 'draft' | 'uncollectible' | 'void' | 'past_due';

  const STATUS_MAP: Record<InvoiceStatus, { kind: StatusKind; label: string }> = {
    paid:          { kind: 'success', label: 'Paid'          },
    open:          { kind: 'warning', label: 'Due'           },
    draft:         { kind: 'neutral', label: 'Draft'         },
    uncollectible: { kind: 'danger',  label: 'Overdue'       },
    void:          { kind: 'neutral', label: 'Void'          },
    past_due:      { kind: 'danger',  label: 'Past due'      },
  };

  let {
    status,
    class: cls = '',
  }: {
    status: InvoiceStatus;
    class?: string;
  } = $props();

  const mapped = $derived(STATUS_MAP[status] ?? { kind: 'neutral' as StatusKind, label: status });
</script>

<StatusBadge status={mapped.kind} label={mapped.label} class={cls} />
