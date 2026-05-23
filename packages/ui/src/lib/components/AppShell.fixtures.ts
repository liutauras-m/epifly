import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'AppShell',
  note: 'Layout container with container-query breakpoints. Slots: topbar, sidebar, main, composer, overlay. Resize the viewport to see breakpoints.',
  cases: [
    { label: 'default (no slots)', props: {} },
  ],
};
export default fixtures;
