import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'StatusBadge',
  note: 'Generic status indicator. Billing-specific labels live in features/billing/InvoiceStatusBadge.',
  cases: [
    { label: 'success',  props: { status: 'success', label: 'Paid' } },
    { label: 'warning',  props: { status: 'warning', label: 'Due' } },
    { label: 'danger',   props: { status: 'danger',  label: 'Overdue' } },
    { label: 'neutral',  props: { status: 'neutral', label: 'Pending' } },
    { label: 'info',     props: { status: 'info',    label: 'Processing' } },
  ],
};
export default fixtures;
