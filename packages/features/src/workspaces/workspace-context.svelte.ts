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
