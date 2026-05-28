/**
 * Svelte context for the currently selected workspace node.
 *
 * Set by (app)/+layout.svelte so any page in the app group can read the
 * selection without prop-drilling through SvelteKit's layout/page boundary.
 */
import { getContext, setContext } from "svelte";

const WS_NODE_CTX = Symbol("workspace-node-ctx");

type WorkspaceNodeContext = { readonly current: string | null };

/**
 * Call from (app)/+layout.svelte.
 * Pass a reactive getter so the context stays live as the user selects nodes.
 */
export function setWorkspaceNodeContext(getNodeId: () => string | null): void {
  const ctx: WorkspaceNodeContext = {
    get current() { return getNodeId(); }
  };
  setContext(WS_NODE_CTX, ctx);
}

/**
 * Call from any page inside (app)/ to read the active workspace node id.
 * Returns `{ current: null }` if no context is set (safe fallback).
 */
export function getWorkspaceNodeContext(): WorkspaceNodeContext {
  return getContext<WorkspaceNodeContext>(WS_NODE_CTX) ?? { current: null };
}

// ── Active thread node context (Step 1.4 / 1.5) ────────────────────────────

const THREAD_NODE_CTX = Symbol("thread-node-ctx");

/**
 * Carries the workspace metadata of the *currently open thread* so that chat
 * pages can render a breadcrumb (Step 1.4/1.5) and feed ambient context to
 * the chat stream (Step 6.1) without prop-drilling through SvelteKit's
 * layout/page boundary.
 */
export type ActiveThreadNodeContext = {
  /** Virtual path of the thread's workspace location, e.g. "Clients/Acme/Kickoff". */
  readonly virtualPath: string | null;
  /** Human-readable workspace folder name (last segment of virtualPath). */
  readonly placeName: string | null;
  /**
   * Step 6.1 — The workspace node id of the **parent folder** that contains
   * this thread. Passed as `workspaceNodeId` to `sdk.chat.stream` so the
   * agent sees the folder's context. Null for root-level threads.
   */
  readonly folderNodeId: string | null;
};

export function setActiveThreadNodeContext(
  getNode: () => { virtualPath?: string | null; name?: string; parentId?: string | null } | null
): void {
  const ctx: ActiveThreadNodeContext = {
    get virtualPath() {
      const n = getNode();
      return n?.virtualPath ?? null;
    },
    get placeName() {
      const n = getNode();
      if (!n?.virtualPath) return null;
      const segs = n.virtualPath.split("/").filter((s) => s.trim().length > 0);
      // The parent folder name is all segments except the last (which is the thread name itself).
      return segs.length > 1 ? segs[segs.length - 2] : segs[0] ?? null;
    },
    get folderNodeId() {
      return getNode()?.parentId ?? null;
    },
  };
  setContext(THREAD_NODE_CTX, ctx);
}

export function getActiveThreadNodeContext(): ActiveThreadNodeContext {
  return (
    getContext<ActiveThreadNodeContext>(THREAD_NODE_CTX) ?? {
      virtualPath: null,
      placeName: null,
      folderNodeId: null,
    }
  );
}

// ── Workspace actions context (Step 7.1) ───────────────────────────────────

const WS_ACTIONS_CTX = Symbol("workspace-actions-ctx");

/**
 * Write-side workspace context for chat pages.
 * Lets new-chat and thread pages notify the workspace tree of optimistic events
 * without prop-drilling through SvelteKit's layout/page boundary.
 */
/** A filing suggestion produced by the heuristic (Step 3.4). */
export type FilingHint = {
  /** The target folder node id to move the thread into. */
  readonly id: string;
  /** Human-readable path for display, e.g. "Clients / Acme". */
  readonly virtualPath: string;
  /** Display name (last path segment or full path). */
  readonly name: string;
};

export type WorkspaceActionsContext = {
  /**
   * Insert a syncing placeholder node for a brand-new thread while its
   * backend projection is in progress. Idempotent — safe to call more than once.
   */
  readonly insertOptimisticThread: (threadId: string, name: string) => void;
  /**
   * Step 1.4 — Select the sidebar tree node whose virtualPath matches the given
   * partial path. Used by ChatBreadcrumb crumb-click to open a folder.
   */
  readonly selectNodeByPath: (virtualPath: string) => void;
  /**
   * Step 3.4 — Reactive getter for the current filing hint (null = no suggestion).
   * Returns a folder to suggest moving the active unsorted thread into.
   */
  readonly getFilingHint: () => FilingHint | null;
  /** Step 3.4 — Dismiss the current hint (user chose Ignore). */
  readonly dismissFilingHint: () => void;
  /** Step 3.4 — Confirm: move the active thread to targetNodeId and dismiss. */
  readonly confirmFiling: (targetNodeId: string, targetVirtualPath: string) => Promise<void>;
};

export function setWorkspaceActionsContext(actions: WorkspaceActionsContext): void {
  setContext(WS_ACTIONS_CTX, actions);
}

export function getWorkspaceActionsContext(): WorkspaceActionsContext | null {
  return getContext<WorkspaceActionsContext>(WS_ACTIONS_CTX) ?? null;
}
