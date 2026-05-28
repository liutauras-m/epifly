/**
 * Smart Views store — retrieval-by-state and retrieval-by-time.
 *
 * Smart Views are *filters, not folders*: they show a flat filtered result set
 * from the same workspace nodes that also live in the Tree and Recents lanes.
 * The same node_id identity is preserved — selecting a result in a Smart View
 * is identical to selecting it from the Tree.
 *
 * Phase 2 ships two views:
 *  - "unsorted"         : thread nodes in the default projection folder
 *                         ("Conversations/" — not yet filed by the user)
 *  - "recently-updated" : all nodes sorted by last_modified desc
 *
 * Phase 5 adds "paused": uses `?paused=true` on filterNodes (backend Step 5.3).
 */

import type { ConusSdk } from "@conusai/sdk";
import type { WorkspaceNode } from "@conusai/types";
import { toSidebarWorkspaceNode } from "./workspace-adapters.js";
import type { SidebarWorkspaceNode } from "./workspace-adapters.js";

export type SmartViewKind = "unsorted" | "recently-updated" | "paused" | "needs-review";

/** Default projection folder path — threads land here until the user files them. */
const DEFAULT_PROJECTION_FOLDER = "Conversations";

export function createSmartViewsStore(sdk: ConusSdk) {
  let activeView = $state<SmartViewKind | null>(null);
  let results = $state<SidebarWorkspaceNode[]>([]);
  let isLoading = $state(false);
  let error = $state<string | null>(null);

  /** Load the given view and set it as active. Clears previous results first. */
  async function selectView(kind: SmartViewKind) {
    activeView = kind;
    isLoading = true;
    error = null;
    results = [];

    const raw = await fetchView(kind);
    isLoading = false;

    if (raw === null) return; // error already set

    results = raw.map((n) => toSidebarWorkspaceNode(n));
  }

  /** Clear the active view (return to tree). */
  function clearView() {
    activeView = null;
    results = [];
    error = null;
  }

  async function fetchView(kind: SmartViewKind): Promise<WorkspaceNode[] | null> {
    switch (kind) {
      case "unsorted": {
        // All thread nodes — then filter client-side to those in the default folder.
        const res = await sdk.workspaces.filterNodes({ kind: "thread", limit: 100 });
        if (res.error) {
          error = res.error.message;
          return null;
        }
        return res.data.filter((n) => isUnsorted(n));
      }

      case "recently-updated": {
        // All nodes (no kind filter) sorted by last_modified desc.
        const res = await sdk.workspaces.filterNodes({ limit: 50 });
        if (res.error) {
          error = res.error.message;
          return null;
        }
        return [...res.data].sort((a, b) => {
          const bt = Date.parse(b.last_modified ?? "");
          const at = Date.parse(a.last_modified ?? "");
          return (Number.isFinite(bt) ? bt : 0) - (Number.isFinite(at) ? at : 0);
        });
      }

      case "paused": {
        // Step 5.3 — uses `?paused=true` to fetch thread nodes where hidden_at IS NOT NULL.
        // The backend filter_nodes handler inverts the default hidden_at exclusion when this
        // param is set, returning only paused (soft-deleted) thread projections.
        const res = await sdk.workspaces.filterNodes({ kind: "thread", paused: true, limit: 100 });
        if (res.error) {
          error = res.error.message;
          return null;
        }
        return res.data;
      }

      case "needs-review": {
        // Phase 8.3 — threads explicitly flagged with metadata.status = "needs-review".
        // This is a concrete trigger (explicit user flag), never a vague heuristic.
        const res = await sdk.workspaces.filterNodes({ kind: "thread", limit: 100 });
        if (res.error) {
          error = res.error.message;
          return null;
        }
        return res.data.filter((n) => {
          const meta = (n.metadata ?? {}) as Record<string, unknown>;
          return meta.status === "needs-review";
        });
      }
    }
  }

  return {
    get activeView() { return activeView; },
    get results() { return results; },
    get isLoading() { return isLoading; },
    get error() { return error; },
    selectView,
    clearView,
  };
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * A thread node is "unsorted" if it has not been filed into a user-created folder.
 * Proxy: its virtual_path starts with the default projection folder, which means
 * the user hasn't moved it out of the default landing zone.
 */
function isUnsorted(node: WorkspaceNode): boolean {
  const path = node.virtual_path ?? "";
  return (
    path === DEFAULT_PROJECTION_FOLDER ||
    path.startsWith(`${DEFAULT_PROJECTION_FOLDER}/`) ||
    // Root-level threads (no "/" — directly at root, no folder context)
    !path.includes("/")
  );
}
