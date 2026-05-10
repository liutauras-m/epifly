import type { WorkspaceNode } from "@conusai/types";
import type { ConusaiClient } from "./client.js";

export function workspaces(client: ConusaiClient) {
  return {
    create(node: Partial<WorkspaceNode>): Promise<WorkspaceNode> {
      return client.request("POST", "/v1/workspaces", node);
    },

    tree(parentId?: string): Promise<WorkspaceNode[]> {
      const qs = parentId ? `?parent_id=${parentId}` : "";
      return client.request("GET", `/v1/workspaces${qs}`);
    },

    get(id: string): Promise<WorkspaceNode> {
      return client.request("GET", `/v1/workspaces/${id}`);
    },
  };
}
