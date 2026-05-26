import type { ConusSdk } from "@conusai/sdk";
import type { WorkspaceNode } from "@conusai/types";

export function createWorkspacesStore(sdk: ConusSdk) {
  let tree = $state<WorkspaceNode[]>([]);
  let isLoading = $state(false);
  let error = $state<string | null>(null);
  let selectedNodeId = $state<string | null>(null);

  async function loadTree(parentId?: string | null) {
    isLoading = true;
    error = null;
    const result = await sdk.workspaces.tree(parentId);
    isLoading = false;
    if (result.error) {
      error = result.error.message;
    } else {
      tree = result.data;
    }
  }

  function selectNode(id: string | null) {
    selectedNodeId = id;
  }

  return {
    get tree() { return tree; },
    get isLoading() { return isLoading; },
    get error() { return error; },
    get selectedNodeId() { return selectedNodeId; },
    loadTree,
    selectNode
  };
}
