import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'ToastHost',
  note: 'Live-renders the global toast singleton. Use the buttons below to push toasts.',
  cases: [
    { label: 'Host (empty state)', props: {} },
  ],
};
export default fixtures;
