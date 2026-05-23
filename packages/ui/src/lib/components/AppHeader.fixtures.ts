import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'AppHeader',
  note: 'Adaptive app topbar (Phase 3.3). Responds to app-shell container query. Adapts compact/medium/expanded.',
  cases: [
    { label: 'default (no slots)', props: {} },
    { label: 'with title text',    props: { class: '' } },
  ],
};
export default fixtures;
