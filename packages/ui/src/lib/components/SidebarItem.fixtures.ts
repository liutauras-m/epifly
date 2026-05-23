import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'SidebarItem',
  note: 'Single nav row — renders <a> or <button>. Icon-only in medium density, icon+label in expanded (Phase 3.4).',
  cases: [
    { label: 'button (inactive)', props: { onclick: () => {} } },
    { label: 'button (active)',   props: { onclick: () => {}, active: true } },
    { label: 'link',              props: { href: '#' } },
    { label: 'link (active)',     props: { href: '#', active: true } },
  ],
};
export default fixtures;
