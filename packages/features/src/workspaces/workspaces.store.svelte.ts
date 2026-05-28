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
      // Step 1.3: do NOT sort here — tree order is user-owned (stable, spatial).
      // Recency sort lives only in the Recents lane (threads store).
      const nodes = result.data;
      tree = parentId ? setChildren(tree, parentId, nodes) : preserveLoadedBranches(nodes, tree);
      hasLoaded = true;
    }
  }

  // Track which parent IDs are currently being loaded to prevent duplicate fetches.
  const loadingChildren = new Set<string>();

  async function loadChildren(parentId: string) {
    if (loadingChildren.has(parentId)) return;
    loadingChildren.add(parentId);
    const result = await sdk.workspaces.tree(parentId);
    loadingChildren.delete(parentId);
    if (result.error) {
      error = result.error.message;
      return;
    }

    // Step 1.3: no sort — tree order is user-owned.
    tree = setChildren(tree, parentId, result.data);
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

  /**
   * Pause (soft-delete) a node.
   * For Thread-kind nodes: backend sets hidden_at (pause, not destroy).
   * For other nodes: hard delete.
   * Step 5.2 — optimistic remove from tree; UI should show "Restore" toast for threads.
   */
  async function deleteNode(nodeId: string, isThread = false) {
    const snapshot = tree;
    tree = removeNode(tree, nodeId);

    const result = await sdk.workspaces.delete(nodeId);
    if (result.error) {
      tree = snapshot;
      error = result.error.message;
      return { error: result.error.message };
    }
    return { data: null, wasThread: isThread };
  }

  /**
   * Restore a paused thread projection (clear hidden_at).
   * Step 5.2 — triggers a tree reload after restore so the node reappears.
   * `threadId` = WorkspaceNode.source_id (the originating thread, not the node ID).
   */
  async function restoreThread(threadId: string) {
    const result = await sdk.workspaces.restoreThread(threadId);
    if (result.error) {
      error = result.error.message;
      return { error: result.error.message };
    }
    // Reload tree so the restored node reappears.
    await loadTree(null);
    return { data: null };
  }

  /**
   * Move a node to a new parent (optimistic). Reverts on API error.
   * Never moves a folder into itself.
   * Step 3.1 — called by DnD drop + "Move to…" menu.
   */
  async function moveNode(
    nodeId: string,
    newParentId: string | null,
    newParentPath: string | null
  ) {
    const node = findNode(tree, nodeId);
    if (!node) return { error: "Node not found" };
    if (newParentId === nodeId) return { error: "Cannot move a folder into itself" };

    // Snapshot for revert
    const snapshot = tree;

    // Optimistic: remove from old location, insert at new
    tree = removeNode(tree, nodeId);
    tree = insertNodeAtTop(tree, node, newParentId);

    const result = await sdk.workspaces.move(nodeId, {
      new_parent_id: newParentId,
      new_parent_path: newParentPath,
    });

    if (result.error) {
      tree = snapshot; // revert
      error = result.error.message;
      return { error: result.error.message };
    }

    // Refresh the parent to get the server-side virtual_path update
    if (newParentId) await loadChildren(newParentId);
    return { data: result.data };
  }

  /**
   * Rename a node in place (optimistic). Reverts on error.
   * Step 3.2 — called by double-click inline input and command palette.
   */
  async function renameNode(nodeId: string, newName: string) {
    const trimmed = newName.trim();
    if (!trimmed) return { error: "Name cannot be empty" };

    const snapshot = tree;
    tree = updateNodeName(tree, nodeId, trimmed);

    const result = await sdk.workspaces.rename(nodeId, trimmed);
    if (result.error) {
      tree = snapshot; // revert
      error = result.error.message;
      return { error: result.error.message };
    }
    return { data: result.data };
  }

  /** Remove a node from the tree (any depth). */
  function removeNode(nodes: WorkspaceTreeNode[], id: string): WorkspaceTreeNode[] {
    return nodes
      .filter((n) => n.id !== id)
      .map((n) =>
        n.children ? { ...n, children: removeNode(n.children, id) } : n
      );
  }

  /** Update just the name of a node (any depth). */
  function updateNodeName(nodes: WorkspaceTreeNode[], id: string, name: string): WorkspaceTreeNode[] {
    return nodes.map((n) => {
      if (n.id === id) return { ...n, name };
      if (n.children) return { ...n, children: updateNodeName(n.children, id, name) };
      return n;
    });
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
    deleteNode,
    restoreThread,
    moveNode,
    renameNode,
    connectRealtime,
    disconnectRealtime
  };
}
