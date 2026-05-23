import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'QuotaBanner',
  note: 'Connects to a real EventSource at apiBase/v1/quota/stream. Use mock apiBase in gallery.',
  cases: [
    { label: 'Default (no stream)', props: { apiBase: '', upgradeUrl: '/account/billing' } },
  ],
};
export default fixtures;
