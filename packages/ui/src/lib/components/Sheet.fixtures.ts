import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'Sheet',
  note: 'Bottom modal on compact, centered dialog on expanded (Phase 3.2). Native <dialog>, drag handle, optional title.',
  cases: [
    { label: 'open (no title)',   props: { open: true,  label: 'Options',    onclose: () => {} } },
    { label: 'open (with title)', props: { open: true,  label: 'Pick color', title: 'Pick color', onclose: () => {} } },
    { label: 'closed',            props: { open: false, label: 'Options',    onclose: () => {} } },
  ],
};
export default fixtures;
