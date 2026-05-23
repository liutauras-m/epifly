import type { ComponentFixtureSet } from '../gallery.types.js';

const nodes = [
  { id: '1', name: 'Project Alpha', kind: 'folder' as const, path: '', metadata: {} },
  { id: '2', name: 'README.md',     kind: 'file'   as const, path: '', metadata: {} },
  { id: '3', name: 'Chat session',  kind: 'conversation' as const, path: '', metadata: {} },
];

const fixtures: ComponentFixtureSet = {
  label: 'WorkspaceTree',
  note: 'Workspace nav tree — renders folder/file/conversation nodes. Used inside Sidebar.',
  cases: [
    { label: 'empty',       props: { nodes: [] } },
    { label: 'with nodes',  props: { nodes, selectedId: '2' } },
  ],
};
export default fixtures;
