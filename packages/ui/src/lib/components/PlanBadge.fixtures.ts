import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'PlanBadge',
  cases: [
    { label: 'Free / active',            props: { tier: 'free',       status: 'active'   } },
    { label: 'Pro / active',             props: { tier: 'pro',        status: 'active'   } },
    { label: 'Enterprise / active',      props: { tier: 'enterprise', status: 'active'   } },
    { label: 'Pro / past_due',           props: { tier: 'pro',        status: 'past_due' } },
    { label: 'Pro / canceled',           props: { tier: 'pro',        status: 'canceled' } },
  ],
};
export default fixtures;
