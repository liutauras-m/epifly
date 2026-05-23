import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'PlanCard',
  cases: [
    {
      label: 'Free tier',
      props: {
        planKey: 'free',
        displayName: 'Free',
        monthlyPriceCents: 0,
        features: ['5 000 tokens / day', '3 capabilities', 'Community support'],
      },
    },
    {
      label: 'Pro tier',
      props: {
        planKey: 'pro',
        displayName: 'Pro',
        monthlyPriceCents: 2000,
        features: ['500 000 tokens / day', 'Unlimited capabilities', 'Priority support', 'Custom integrations'],
        current: false,
      },
    },
    {
      label: 'Pro (current)',
      props: {
        planKey: 'pro',
        displayName: 'Pro',
        monthlyPriceCents: 2000,
        features: ['500 000 tokens / day', 'Unlimited capabilities', 'Priority support'],
        current: true,
      },
    },
    {
      label: 'Enterprise',
      props: {
        planKey: 'enterprise',
        displayName: 'Enterprise',
        monthlyPriceCents: 0,
        features: ['Custom token limits', 'Dedicated infrastructure', 'SLA', 'SAML SSO'],
      },
    },
  ],
};
export default fixtures;
