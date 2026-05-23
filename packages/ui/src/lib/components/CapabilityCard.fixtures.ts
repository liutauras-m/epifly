import type { ComponentFixtureSet } from '../gallery.types.js';

const fixtures: ComponentFixtureSet = {
  label: 'CapabilityCard',
  cases: [
    {
      label: 'Function tool',
      props: {
        card: {
          capability_id: 'cap-001',
          name: 'web_search',
          description: 'Search the web and return summarised results with citations.',
          kind: 'function',
          tenant_scope: ['acme-corp'],
          tags: ['search', 'web'],
        },
      },
    },
    {
      label: 'Browser automation',
      props: {
        card: {
          capability_id: 'cap-002',
          name: 'browse_page',
          description: 'Open a URL in a headless browser and extract structured data.',
          kind: 'browser',
          tenant_scope: [],
          tags: ['browser', 'scraping'],
        },
      },
    },
    {
      label: 'Code interpreter',
      props: {
        card: {
          capability_id: 'cap-003',
          name: 'run_python',
          description: 'Execute Python code in a sandboxed environment and return stdout.',
          kind: 'code',
          tenant_scope: ['acme-corp', 'beta-corp'],
          tags: ['code', 'python'],
        },
      },
    },
  ],
};
export default fixtures;
