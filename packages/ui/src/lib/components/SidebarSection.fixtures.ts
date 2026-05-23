import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'SidebarSection',
  note: 'Labeled section within Sidebar (Phase 3.4). Eyebrow hides on icon-only (768–1023px) via container query.',
  cases: [
    { label: 'with eyebrow',    props: { eyebrow: 'RECENT' } },
    { label: 'without eyebrow', props: {} },
  ],
};
export default fixtures;
