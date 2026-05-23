import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'Sidebar',
  note: 'Adaptive nav rail (Phase 3.4). Density driven by app-shell container query. Supports search/footer slots.',
  cases: [
    { label: 'default (empty)',    props: {} },
    { label: 'with class',        props: { class: 'sidebar-demo' } },
  ],
};
export default fixtures;
