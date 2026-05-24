import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'PageHeader',
  cases: [
    {
      label: 'Title only',
      props: { title: 'Account' },
    },
    {
      label: 'Eyebrow + title',
      props: { eyebrow: 'SETTINGS', title: 'Account' },
    },
    {
      label: 'Full — eyebrow + title + subtitle',
      props: {
        eyebrow:  'BILLING',
        title:    'Billing & Plans',
        subtitle: 'Manage your subscription, upgrade, or view invoices.',
      },
    },
    {
      label: 'Usage page style',
      props: { eyebrow: 'USAGE', title: 'Usage & Quotas' },
    },
  ],
};

export default fixtures;
