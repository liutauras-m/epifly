import type { WorkspaceNode } from "@conusai/types";

/**
 * Currently-active workspace node for breadcrumb / context display.
 * Drives the breadcrumb row on mobile and the header path on web.
 */
let node = $state<WorkspaceNode | null>(null);

export const breadcrumbsStore = {
  get node() {
    return node;
  },
  set(n: WorkspaceNode | null) {
    node = n;
  },
  clear() {
    node = null;
  },
};
