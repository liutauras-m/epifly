import type { ConusSdk } from "@conusai/sdk";
import type { WorkspaceNode } from "@conusai/types";

type WorkspaceTreeNode = WorkspaceNode & { children?: WorkspaceTreeNode[] };

export function createWorkspacesStore(sdk: ConusSdk) {
  let tree = $state<WorkspaceTreeNode[]>([]);
  let isLoading = $state(false);
  let isCreating = $state(false);
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
      const sortedNodes = sortRecentFirst(result.data);
      tree = parentId ? setChildren(tree, parentId, sortedNodes) : preserveLoadedBranches(sortedNodes, tree);
      hasLoaded = true;
    }
  }

  async function loadChildren(parentId: string) {
    if (isLoading) return;
    isLoading = true;
    error = null;
    const result = await sdk.workspaces.tree(parentId);
    isLoading = false;
    if (result.error) {
      error = result.error.message;
      return;
    }

    tree = setChildren(tree, parentId, sortRecentFirst(result.data));
  }

  /** Load only if not already loaded. Use for initial mount. */
  async function loadTreeOnce(parentId?: string | null) {
    if (hasLoaded || isLoading) return;
    return loadTree(parentId);
  }

  function selectNode(id: string | null) {
    selectedNodeId = id;
  }

  async function selectAndLoadNode(id: string) {
    selectedNodeId = id;
    const node = findNode(tree, id);
    if (node?.kind === "folder") await loadChildren(id);
  }

  async function createNode(kind: "folder" | "document", name: string, parentId?: string | null) {
    const trimmed = name.trim();
    if (!trimmed || isCreating) return null;

    isCreating = true;
    error = null;
    const apiKind = kind === "document" ? "conversation" : "folder";
    const apiName = apiKind === "conversation" && !trimmed.endsWith(".md") ? `${trimmed}.md` : trimmed;
    const result = await sdk.workspaces.create({
      kind: apiKind,
      name: apiName,
      parent_id: parentId ?? null
    });
    isCreating = false;

    if (result.error) {
      error = result.error.message;
      return null;
    }

    const node = result.data;
    tree = insertNodeAtTop(tree, node, parentId ?? null);
    selectedNodeId = node.id;
    return node;
  }

  function findNode(nodes: WorkspaceTreeNode[], id: string): WorkspaceTreeNode | null {
    for (const node of nodes) {
      if (node.id === id) return node;
      const child = node.children ? findNode(node.children, id) : null;
      if (child) return child;
    }
    return null;
  }

  function setChildren(nodes: WorkspaceTreeNode[], parentId: string, children: WorkspaceNode[]): WorkspaceTreeNode[] {
    return nodes.map((node) => {
      if (node.id === parentId) return { ...node, children };
      if (node.children) return { ...node, children: setChildren(node.children, parentId, children) };
      return node;
    });
  }

  function preserveLoadedBranches(nodes: WorkspaceNode[], previousNodes: WorkspaceTreeNode[]): WorkspaceTreeNode[] {
    return nodes.map((node) => {
      const previous = findNode(previousNodes, node.id);
      return previous?.children ? { ...node, children: previous.children } : node;
    });
  }

  function insertNodeAtTop(nodes: WorkspaceTreeNode[], node: WorkspaceNode, parentId: string | null): WorkspaceTreeNode[] {
    if (!parentId) return [node, ...nodes.filter((existing) => existing.id !== node.id)];

    return nodes.map((existing) => {
      if (existing.id === parentId) {
        const children = existing.children ?? [];
        return { ...existing, children: [node, ...children.filter((child) => child.id !== node.id)] };
      }
      if (existing.children) return { ...existing, children: insertNodeAtTop(existing.children, node, parentId) };
      return existing;
    });
  }

  function sortRecentFirst(nodes: WorkspaceNode[]): WorkspaceNode[] {
    return [...nodes].sort((left, right) => {
      const rightTime = Date.parse(right.last_modified ?? "");
      const leftTime = Date.parse(left.last_modified ?? "");
      return (Number.isFinite(rightTime) ? rightTime : 0) - (Number.isFinite(leftTime) ? leftTime : 0);
    });
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
        await refreshLoadedTree();
      } while (pendingRealtimeRefresh);
    } finally {
      realtimeRefreshInFlight = false;
    }
  }

  async function refreshLoadedTree() {
    const loadedFolderIds = collectLoadedFolderIds(tree);
    await loadTree(null);

    for (const folderId of loadedFolderIds) {
      const node = findNode(tree, folderId);
      if (node?.kind === "folder") await loadChildren(folderId);
    }

    if (selectedNodeId && !findNode(tree, selectedNodeId)) selectedNodeId = null;
  }

  function collectLoadedFolderIds(nodes: WorkspaceTreeNode[]): string[] {
    return nodes.flatMap((node) => {
      const nested = node.children ? collectLoadedFolderIds(node.children) : [];
      return node.kind === "folder" && node.children ? [node.id, ...nested] : nested;
    });
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
    get isCreating() { return isCreating; },
    get hasLoaded() { return hasLoaded; },
    get error() { return error; },
    get selectedNodeId() { return selectedNodeId; },
    loadTree,
    loadTreeOnce,
    loadChildren,
    selectNode,
    selectAndLoadNode,
    createNode,
    connectRealtime,
    disconnectRealtime
  };
}
