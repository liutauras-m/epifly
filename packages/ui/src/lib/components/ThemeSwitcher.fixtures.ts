import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'ThemeSwitcher',
  note: 'Requires ThemeProvider context — the gallery wraps it automatically.',
  cases: [
    { label: 'Button (context-aware)', props: {} },
  ],
};
export default fixtures;
