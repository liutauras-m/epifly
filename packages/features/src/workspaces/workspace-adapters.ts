/**
 * Adapters from backend WorkspaceNode shapes to the UI sidebar shape.
 *
 * The output type mirrors WorkspaceNode from @epifly/ui/workspace-tree.svelte.
 * Defined locally to keep @epifly/features independent of @epifly/ui.
 * TypeScript structural typing ensures compatibility at call sites.
 */

export type SidebarWorkspaceNode = {
  id: string;
  name: string;
  kind: "folder" | "thread" | "document";
  parentId?: string | null;
  children?: SidebarWorkspaceNode[];
};

type WorkspaceNodeLike = {
  id: string;
  name: string;
  parent_id?: string | null;
  kind: string;
  children?: WorkspaceNodeLike[];
};

export function toSidebarWorkspaceNode(node: WorkspaceNodeLike): SidebarWorkspaceNode {
  return {
    id: node.id,
    name: node.name,
    parentId: node.parent_id ?? null,
    kind: toSidebarWorkspaceKind(node.kind),
    children: node.children?.map((child) => toSidebarWorkspaceNode(child))
  };
}

function toSidebarWorkspaceKind(kind: string): SidebarWorkspaceNode["kind"] {
  switch (kind) {
    case "conversation": return "document";
    case "folder":       return "folder";
    case "document":     return "document";
    case "file":         return "document";
    default:             return "document";
  }
}
