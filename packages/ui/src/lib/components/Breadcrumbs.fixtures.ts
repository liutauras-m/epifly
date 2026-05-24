import type { ComponentFixtureSet } from '../gallery.types.js';
import type { BreadcrumbItem } from './Breadcrumbs.svelte';

const fixtures: ComponentFixtureSet = {
  label: 'Breadcrumbs',
  note: 'Breadcrumb navigation — renders an ordered trail of links with the last item as the current page.',
  cases: [
    {
      label: 'Two levels',
      props: {
        items: [
          { label: 'Account', href: '/account' },
          { label: 'Billing' },
        ] satisfies BreadcrumbItem[],
      },
    },
    {
      label: 'Three levels',
      props: {
        items: [
          { label: 'Home', href: '/' },
          { label: 'Account', href: '/account' },
          { label: 'Usage' },
        ] satisfies BreadcrumbItem[],
      },
    },
    {
      label: 'Single item (root)',
      props: {
        items: [{ label: 'Account' }] satisfies BreadcrumbItem[],
      },
    },
  ],
};

export default fixtures;
