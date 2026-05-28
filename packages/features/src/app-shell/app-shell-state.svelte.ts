import type { ConusSdk } from "@conusai/sdk";
import { createThreadsStore } from "../threads/threads.store.svelte.js";
import { sortByRecent } from "../threads/threads.utils.js";
import { createWorkspacesStore } from "../workspaces/workspaces.store.svelte.js";
import { createSmartViewsStore } from "../workspaces/smart-views.store.svelte.js";
import type { SmartViewKind } from "../workspaces/smart-views.store.svelte.js";
import { toSidebarWorkspaceNode } from "../workspaces/workspace-adapters.js";
import type { SidebarWorkspaceNode } from "../workspaces/workspace-adapters.js";
import { createPeekStore } from "../workspaces/workspace-peek.store.svelte.js";

type Navigate = (href: string) => void | Promise<void>;

export type CreateAppShellStateArgs = {
  sdk: ConusSdk;
  /** Reactive getter for the current pathname — pass `() => page.url.pathname`. */
  getPathname: () => string;
  /** Reactive getter for the active thread id — pass `() => page.params.threadId ?? null`. */
  getThreadId: () => string | null;
  /**
   * Navigation callback injected by the consuming app. Keeps this module free
   * of `$app/navigation` so it works in both `apps/web` and `apps/native`
   * without pulling SvelteKit internals into the shared features package.
   */
  navigate: Navigate;
};

export type { SidebarWorkspaceNode };

/**
 * Reactive shell state for app layouts.
 *
 * Owns sidebar data derivations and navigation actions so that
 * `+layout.svelte` stays thin — compose shell, nothing else.
 *
 * ```svelte
 * const shell = createAppShellState({ sdk, getPathname, getThreadId, navigate: goto });
 * onMount(shell.load);
 * ```
 */
export function createAppShellState(args: CreateAppShellStateArgs) {
  const threadsStore = createThreadsStore(args.sdk);
  const workspacesStore = createWorkspacesStore(args.sdk);
  const smartViewsStore = createSmartViewsStore(args.sdk);
  const peekStore = createPeekStore(args.sdk);

  // Route-derived — reactive because the getters are called inside $derived.
  const activePath = $derived(args.getPathname());
  const activeThreadId = $derived(args.getThreadId());

  // Sidebar data — pure transformations, never side effects.
  const sortedThreads = $derived(sortByRecent(threadsStore.threads));

  // Phase 7.1 — optimistic thread nodes inserted before backend projection arrives.
  // Stored separately from the backend tree to avoid mixing UI state into WorkspaceNode[].
  const optimisticThreadNodes = $state<SidebarWorkspaceNode[]>([]);

  // Backend-only flat list — used for reconciliation without creating a dependency cycle.
  const backendNodes = $derived<SidebarWorkspaceNode[]>(
    workspacesStore.tree.map(toSidebarWorkspaceNode)
  );
  const flatBackendNodes = $derived(flattenNodes(backendNodes));

  // Sidebar data — optimistic nodes at top so they appear immediately.
  const workspaceNodes = $derived<SidebarWorkspaceNode[]>([
    ...optimisticThreadNodes,
    ...backendNodes,
  ]);

  /** Flatten the workspace tree for O(n) lookups. */
  function flattenNodes(nodes: SidebarWorkspaceNode[]): SidebarWorkspaceNode[] {
    const out: SidebarWorkspaceNode[] = [];
    for (const n of nodes) {
      out.push(n);
      if (n.children?.length) out.push(...flattenNodes(n.children));
    }
    return out;
  }

  const flatWorkspaceNodes = $derived(flattenNodes(workspaceNodes));

  /**
   * The workspace node that corresponds to the currently active thread.
   * Used to derive the breadcrumb and ambient context for the chat header.
   */
  const activeThreadNode = $derived(
    activeThreadId
      ? (flatWorkspaceNodes.find((n) => n.kind === "thread" && n.threadId === activeThreadId) ?? null)
      : null
  );

  /**
   * Phase 7.1 — Auto-reconcile: when a real backend node arrives for an optimistic
   * placeholder, remove the placeholder. Reads flatBackendNodes (not flatWorkspaceNodes)
   * to avoid a dependency cycle.
   */
  $effect(() => {
    if (optimisticThreadNodes.length === 0) return;
    const backendThreadIds = new Set(
      flatBackendNodes
        .filter((n) => n.kind === "thread")
        .map((n) => n.threadId)
        .filter((id): id is string => Boolean(id))
    );
    for (let i = optimisticThreadNodes.length - 1; i >= 0; i--) {
      const opt = optimisticThreadNodes[i];
      if (opt.threadId && backendThreadIds.has(opt.threadId)) {
        optimisticThreadNodes.splice(i, 1);
      }
    }
  });

  /**
   * Step 7.1 — Insert a syncing placeholder for a brand-new thread the moment
   * `thread_id` arrives in the chat stream. The node shows a pulsing indicator
   * until the workspace realtime event fires and the real node is reconciled.
   * Idempotent: no-ops if the thread is already in the tree.
   */
  function insertOptimisticThread(threadId: string, name: string): void {
    // Don't insert if the real node is already present.
    const alreadyInBackend = flatBackendNodes.some(
      (n) => n.kind === "thread" && n.threadId === threadId
    );
    if (alreadyInBackend) return;
    // Don't duplicate an existing optimistic entry.
    const alreadyOptimistic = optimisticThreadNodes.some((n) => n.threadId === threadId);
    if (alreadyOptimistic) return;

    optimisticThreadNodes.push({
      id: `optimistic:${threadId}`,
      name,
      kind: "thread",
      threadId,
      virtualPath: name,
      parentId: null,
      tags: [],
      syncing: true,
    });
  }

  /** Call once from the layout's onMount. Stores dedupe internally. */
  function load() {
    threadsStore.loadOnce({ limit: 20 });
    workspacesStore.loadTreeOnce(null);
    threadsStore.connectRealtime();
    workspacesStore.connectRealtime();

    return () => {
      threadsStore.disconnectRealtime();
      workspacesStore.disconnectRealtime();
    };
  }

  // Navigation — delegates to injected navigate, no direct SvelteKit dep.
  function goToNewChat() { args.navigate("/"); }
  function goToThread(id: string) { args.navigate(`/chat/${id}`); }
  function selectWorkspaceNode(id: string) { void workspacesStore.selectAndLoadNode(id); }
  function createWorkspaceNode(kind: "folder" | "document", name: string, parentId?: string | null) {
    return workspacesStore.createNode(kind, name, parentId);
  }
  function moveWorkspaceNode(nodeId: string, newParentId: string | null, newParentPath: string | null) {
    return workspacesStore.moveNode(nodeId, newParentId, newParentPath);
  }
  function renameWorkspaceNode(nodeId: string, newName: string) {
    return workspacesStore.renameNode(nodeId, newName);
  }
  function deleteWorkspaceNode(nodeId: string, isThread = false) {
    return workspacesStore.deleteNode(nodeId, isThread);
  }
  function restoreThread(threadId: string) {
    return workspacesStore.restoreThread(threadId);
  }

  /**
   * Step 1.4 — Select the workspace folder that matches the given virtual path
   * so that clicking a breadcrumb crumb opens the folder in the sidebar tree.
   * Matches `kind: "folder"` nodes whose virtualPath equals the crumb's partial path.
   */
  function selectWorkspaceNodeByPath(virtualPath: string): void {
    const node = flatWorkspaceNodes.find(
      (n) => n.kind === "folder" && n.virtualPath === virtualPath
    );
    if (node) selectWorkspaceNode(node.id);
  }

  /**
   * Phase 8.3 — flag or clear the status of a workspace node.
   * status="needs-review" surfaces the node in the Needs Review smart view.
   */
  function setNodeStatus(nodeId: string, status: string | null) {
    return workspacesStore.setStatus(nodeId, status);
  }

  // ── Step 3.4 — Suggested filing (heuristic v1) ─────────────────────────────
  //
  // Suggest a filing destination when the active thread is unsorted (still in
  // the default Conversations folder) and the user has a folder open/selected
  // in the sidebar tree. INVARIANT: the system NEVER moves without confirmation.
  //
  // Heuristic v1: current selected workspace node (or its nearest folder ancestor).
  // Upgraded later with embeddings / title match once this pattern proves stable.

  /** Whether the user dismissed the hint for the current thread. Reset on navigation. */
  let filingHintDismissed = $state(false);

  // Reset the dismissed flag whenever the active thread changes.
  $effect(() => {
    const _tid = activeThreadId; // reactive dependency
    filingHintDismissed = false;
  });

  /**
   * The suggested folder node for the active thread, or null when there is no
   * useful suggestion (thread is already filed, no folder selected, or dismissed).
   */
  const filingHint = $derived(
    (() => {
      if (filingHintDismissed) return null;
      if (!activeThreadNode) return null;

      // Only suggest when the thread is unsorted.
      const path = activeThreadNode.virtualPath ?? "";
      const isUnsorted =
        path === "Conversations" ||
        path.startsWith("Conversations/") ||
        !path.includes("/");
      if (!isUnsorted) return null;

      // Suggest: the currently selected workspace node (or its nearest folder ancestor).
      const selId = workspacesStore.selectedNodeId;
      if (!selId) return null;
      const selNode = flatWorkspaceNodes.find((n) => n.id === selId);
      if (!selNode) return null;
      if (selNode.kind === "folder") return selNode;
      // Non-folder selection: look up its parent folder.
      if (selNode.parentId) {
        return (
          flatWorkspaceNodes.find(
            (n) => n.id === selNode.parentId && n.kind === "folder"
          ) ?? null
        );
      }
      return null;
    })()
  );

  /** Step 3.4 — Dismiss the filing hint for this thread (user chose Ignore). */
  function dismissFilingHint(): void {
    filingHintDismissed = true;
  }

  /** Step 3.4 — Confirm the suggestion: move the thread and dismiss. */
  async function confirmFiling(targetNodeId: string, targetVirtualPath: string): Promise<void> {
    if (!activeThreadNode) return;
    await moveWorkspaceNode(activeThreadNode.id, targetNodeId, targetVirtualPath);
    filingHintDismissed = true;
  }

  /** Semantic + name search against the backend, returning sidebar-shaped nodes. */
  async function searchWorkspace(query: string): Promise<SidebarWorkspaceNode[]> {
    const trimmed = query.trim();
    if (!trimmed) return [];
    const result = await args.sdk.workspaces.search(trimmed, 30, 'semantic');
    if (result.error || !result.data) return [];
    return result.data.map(toSidebarWorkspaceNode);
  }

  return {
    get activePath() { return activePath; },
    get activeThreadId() { return activeThreadId; },
    get sortedThreads() { return sortedThreads; },
    get workspaceNodes() { return workspaceNodes; },
    get threadsLoading() { return threadsStore.isLoading; },
    get workspaceLoading() { return workspacesStore.isLoading; },
    get workspaceCreating() { return workspacesStore.isCreating; },
    get workspaceError() { return workspacesStore.error; },
    get selectedWorkspaceNodeId() { return workspacesStore.selectedNodeId; },
    /** The workspace node (thread projection) for the active thread, or null. */
    get activeThreadNode() { return activeThreadNode; },
    // Smart Views
    get smartViewActive() { return smartViewsStore.activeView; },
    get smartViewResults() { return smartViewsStore.results; },
    get smartViewLoading() { return smartViewsStore.isLoading; },
    get smartViewError() { return smartViewsStore.error; },
    load,
    goToNewChat,
    goToThread,
    selectWorkspaceNode,
    createWorkspaceNode,
    moveWorkspaceNode,
    renameWorkspaceNode,
    deleteWorkspaceNode,
    restoreThread,
    selectWorkspaceNodeByPath,
    searchWorkspace,
    insertOptimisticThread,
    setNodeStatus,
    selectSmartView: (kind: SmartViewKind) => smartViewsStore.selectView(kind),
    clearSmartView: () => smartViewsStore.clearView(),
    // Step 3.4 — suggested filing
    get filingHint() { return filingHint; },
    dismissFilingHint,
    confirmFiling,
    // Phase 4 — "View as document" peek
    get peekOpen() { return peekStore.isOpen; },
    get peekNodeName() { return peekStore.nodeName; },
    get peekSummary() { return peekStore.summary; },
    get peekContent() { return peekStore.content; },
    get peekLoading() { return peekStore.isLoading; },
    get peekError() { return peekStore.error; },
    // Phase 8.2 — related items resolved from metadata.
    get peekRelatedItems() { return peekStore.relatedItems; },
    openPeek: (nodeId: string, name?: string, summary?: string) => peekStore.open(nodeId, name, summary),
    closePeek: () => peekStore.close(),
    /**
     * Phase 8.2 — navigate to a related workspace item.
     * Threads open as chat; folders/documents select in the tree.
     */
    navigateToRelated(nodeId: string) {
      // Find the node in the flat workspace list to determine kind + threadId.
      const node = flatWorkspaceNodes.find((n) => n.id === nodeId);
      if (node?.kind === "thread" && node.threadId) {
        void args.navigate(`/chat/${node.threadId}`);
      } else if (node) {
        workspacesStore.selectNode(node.id);
      }
      peekStore.close();
    },
  };
}
