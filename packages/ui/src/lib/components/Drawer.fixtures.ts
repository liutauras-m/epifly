import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'Drawer',
  note: 'Edge-slide modal backed by native <dialog> (Phase 3.2). Left/right edge, focus-trapped, Escape/backdrop closes.',
  cases: [
    { label: 'left open',   props: { open: true,  side: 'left',  label: 'Navigation', onclose: () => {} } },
    { label: 'right open',  props: { open: true,  side: 'right', label: 'Details',    onclose: () => {} } },
    { label: 'closed',      props: { open: false,                label: 'Navigation', onclose: () => {} } },
  ],
};
export default fixtures;
