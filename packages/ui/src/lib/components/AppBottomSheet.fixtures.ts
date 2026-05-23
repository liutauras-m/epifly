import type { ComponentFixtureSet } from '../gallery.types.js';
const fixtures: ComponentFixtureSet = {
  label: 'AppBottomSheet',
  note: '@deprecated — migration shim for Sheet (Phase 3.2). Delegates to canonical Sheet component.',
  cases: [
    { label: 'open',   props: { open: true,  title: 'Options',  onClose: () => {} } },
    { label: 'closed', props: { open: false, title: 'Options',  onClose: () => {} } },
  ],
};
export default fixtures;
