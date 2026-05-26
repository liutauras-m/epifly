import type { WorkspaceNode } from "@conusai/types";

/** Returns only direct children of a given parent id. */
export function childrenOf(
  nodes: WorkspaceNode[],
  parentId: string | null
): WorkspaceNode[] {
  return nodes.filter(n => (n.parent_id ?? null) === parentId);
}

/** Builds a path string like "Root / Folder / Subfolder" for a node. */
export function nodePath(
  nodes: WorkspaceNode[],
  nodeId: string,
  separator = " / "
): string {
  const byId = new Map(nodes.map(n => [n.id, n]));
  const parts: string[] = [];
  let current = byId.get(nodeId);
  while (current) {
    parts.unshift(current.name);
    current = current.parent_id ? byId.get(current.parent_id) : undefined;
  }
  return parts.join(separator);
}
