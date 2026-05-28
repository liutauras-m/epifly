/**
 * Peek store — "View as document" for workspace nodes.
 *
 * Fetches the projected Markdown content of a workspace node on demand.
 * The peek panel is read-only (write path stays in the full editor / chat).
 *
 * Usage:
 *   const peek = createPeekStore(sdk);
 *   peek.open("node-id");   // fetch + show
 *   peek.close();           // clear
 */

import type { ConusSdk } from "@conusai/sdk";

export function createPeekStore(sdk: ConusSdk) {
  let nodeId = $state<string | null>(null);
  let content = $state<string | null>(null);
  let nodeName = $state<string | null>(null);
  let summary = $state<string | null>(null);
  let isLoading = $state(false);
  let error = $state<string | null>(null);

  async function open(id: string, name?: string, nodeSummary?: string) {
    if (nodeId === id && content !== null) return; // already loaded

    nodeId = id;
    nodeName = name ?? null;
    summary = nodeSummary ?? null;
    isLoading = true;
    error = null;
    content = null;

    const res = await sdk.workspaces.getContent(id);
    isLoading = false;

    if (res.error) {
      error = res.error.message;
      return;
    }
    content = res.data.content;
  }

  function close() {
    nodeId = null;
    content = null;
    nodeName = null;
    summary = null;
    isLoading = false;
    error = null;
  }

  return {
    get nodeId() { return nodeId; },
    get content() { return content; },
    get nodeName() { return nodeName; },
    get summary() { return summary; },
    get isLoading() { return isLoading; },
    get error() { return error; },
    get isOpen() { return nodeId !== null; },
    open,
    close,
  };
}

export type PeekStore = ReturnType<typeof createPeekStore>;
