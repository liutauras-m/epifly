import type { WorkspaceNode } from "@conusai/types";

/**
 * Currently-active workspace node for breadcrumb / context display.
 * Drives the breadcrumb row on mobile and the header path on web.
 */
export function createBreadcrumbsStore() {
  let node = $state<WorkspaceNode | null>(null);

  return {
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
}

export type BreadcrumbsStore = ReturnType<typeof createBreadcrumbsStore>;
