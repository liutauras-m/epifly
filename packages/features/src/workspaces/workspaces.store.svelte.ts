import type { ConusSdk } from "@conusai/sdk";
import type { WorkspaceNode } from "@conusai/types";

export function createWorkspacesStore(sdk: ConusSdk) {
  let tree = $state<WorkspaceNode[]>([]);
  let isLoading = $state(false);
  let hasLoaded = $state(false);
  let error = $state<string | null>(null);
  let selectedNodeId = $state<string | null>(null);
  let realtimeSocket: WebSocket | null = null;
  let realtimeRefreshInFlight = false;
  let pendingRealtimeRefresh = false;

  async function loadTree(parentId?: string | null) {
    if (isLoading) return;
    isLoading = true;
    error = null;
    const result = await sdk.workspaces.tree(parentId);
    isLoading = false;
    if (result.error) {
      error = result.error.message;
    } else {
      tree = result.data;
      hasLoaded = true;
    }
  }

  /** Load only if not already loaded. Use for initial mount. */
  async function loadTreeOnce(parentId?: string | null) {
    if (hasLoaded || isLoading) return;
    return loadTree(parentId);
  }

  function selectNode(id: string | null) {
    selectedNodeId = id;
  }

  async function waitForIdleLoad() {
    while (isLoading) {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
  }

  async function refreshFromRealtime() {
    if (realtimeRefreshInFlight) {
      pendingRealtimeRefresh = true;
      return;
    }

    realtimeRefreshInFlight = true;
    try {
      do {
        pendingRealtimeRefresh = false;
        await waitForIdleLoad();
        await loadTree(null);
      } while (pendingRealtimeRefresh);
    } finally {
      realtimeRefreshInFlight = false;
    }
  }

  function isWorkspaceChangeMessage(data: unknown) {
    if (!data || typeof data !== "object") return false;
    const record = data as Record<string, unknown>;
    return (
      (typeof record.op === "string" && record.op.startsWith("workspace.")) ||
      record.resource === "workspace" ||
      record.type === "workspace"
    );
  }

  function connectRealtime() {
    if (realtimeSocket) return;

    realtimeSocket = sdk.realtime.subscribe();
    realtimeSocket.addEventListener("message", (event) => {
      try {
        const data = JSON.parse(typeof event.data === "string" ? event.data : "{}");
        if (isWorkspaceChangeMessage(data)) void refreshFromRealtime();
      } catch {
        // Ignore malformed realtime messages.
      }
    });
  }

  function disconnectRealtime() {
    realtimeSocket?.close();
    realtimeSocket = null;
  }

  return {
    get tree() { return tree; },
    get isLoading() { return isLoading; },
    get hasLoaded() { return hasLoaded; },
    get error() { return error; },
    get selectedNodeId() { return selectedNodeId; },
    loadTree,
    loadTreeOnce,
    selectNode,
    connectRealtime,
    disconnectRealtime
  };
}
