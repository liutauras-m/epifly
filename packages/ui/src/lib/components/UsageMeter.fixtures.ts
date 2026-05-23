import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'UsageMeter',
  cases: [
    { label: 'Empty (0%)',       props: { label: 'Tokens today',   used: 0,      limit: 5000,  unit: 'tokens' } },
    { label: 'Half (50%)',       props: { label: 'Tokens today',   used: 2500,   limit: 5000,  unit: 'tokens' } },
    { label: 'Warning (82%)',    props: { label: 'Tokens today',   used: 4100,   limit: 5000,  unit: 'tokens' } },
    { label: 'Exceeded (100%)', props: { label: 'Tokens today',   used: 5000,   limit: 5000,  unit: 'tokens' } },
    { label: 'Over limit',      props: { label: 'Requests',        used: 120,    limit: 100,   unit: 'req'    } },
    { label: 'No limit',        props: { label: 'Storage',         used: 842,    limit: null,  unit: 'MB'     } },
  ],
};
export default fixtures;
