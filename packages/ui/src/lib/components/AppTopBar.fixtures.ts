import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'AppTopBar',
  note: 'Renders with snippet slots (rightAction, children). Previewed with static text stand-ins.',
  cases: [
    { label: 'Default (title only)', props: { title: 'ConusAI' } },
    { label: 'With back button',     props: { title: 'Account', canGoBack: true } },
    { label: 'Custom title',         props: { title: 'Capabilities' } },
  ],
};
export default fixtures;
