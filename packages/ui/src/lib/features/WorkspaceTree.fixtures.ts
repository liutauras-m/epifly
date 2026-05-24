import type { ComponentFixtureSet } from '../gallery.types.js';

/**
 * WorkspaceTree fixtures (features/ canonical).
 *
 * The full component requires a live ConusSdk so fixture cases pass
 * minimal SDK stubs — just enough for the gallery renderer to show
 * the loading/empty states without a real backend.
 */
const fixtures: ComponentFixtureSet = {
  label: 'WorkspaceTree',
  note: 'Requires ConusSdk — shows loading state in the gallery. Use the shell preview for full interaction.',
  cases: [
    {
      label: 'loading state (stub SDK)',
      props: {
        sdk: {
          workspaces: {
            tree: () => new Promise(() => {}), // never resolves → stays in loading state
          },
        },
      },
    },
    {
      label: 'empty state (stub SDK)',
      props: {
        sdk: {
          workspaces: {
            tree: () => Promise.resolve({ data: { nodes: [] }, error: null }),
          },
        },
      },
    },
  ],
};

export default fixtures;
