import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'DataTable',
  cases: [
    {
      label: 'Invoice table (basic)',
      props: {
        caption: 'Invoices',
        columns: [
          { key: 'date',   label: 'Date',   sortable: true },
          { key: 'amount', label: 'Amount', sortable: true, align: 'right' },
          { key: 'status', label: 'Status' },
        ],
        rows: [
          { date: '2026-05-01', amount: '$99.00', status: 'Paid' },
          { date: '2026-04-01', amount: '$99.00', status: 'Paid' },
          { date: '2026-03-01', amount: '$99.00', status: 'Past due' },
        ],
      },
    },
    {
      label: 'Empty state',
      props: {
        columns: [
          { key: 'date',   label: 'Date' },
          { key: 'amount', label: 'Amount' },
        ],
        rows: [],
        emptyMessage: 'No invoices yet.',
      },
    },
  ],
};

export default fixtures;
