import type { ComponentFixtureSet } from '../gallery.types.js';
const fixtures: ComponentFixtureSet = {
  label: 'AppDrawer',
  note: '@deprecated — migration shim for Drawer (Phase 3.2). Delegates to canonical Drawer component.',
  cases: [
    { label: 'open',   props: { open: true,  onClose: () => {} } },
    { label: 'closed', props: { open: false, onClose: () => {} } },
  ],
};
export default fixtures;
