/**
 * Peek store — "View as document" for workspace nodes.
 *
 * Fetches the projected Markdown content of a workspace node on demand.
 * The peek panel is read-only (write path stays in the full editor / chat).
 *
 * Phase 8.2: also loads related items from `metadata.related_node_ids` and
 * `metadata.linked_file_ids`, resolving names via parallel `sdk.workspaces.get` calls.
 *
 * Usage:
 *   const peek = createPeekStore(sdk);
 *   peek.open("node-id");   // fetch + show
 *   peek.close();           // clear
 */

import type { ConusSdk } from "@conusai/sdk";

export type PeekRelatedItem = {
  id: string;
  name: string;
  kind: "folder" | "thread" | "document";
};

export function createPeekStore(sdk: ConusSdk) {
  let nodeId = $state<string | null>(null);
  let content = $state<string | null>(null);
  let nodeName = $state<string | null>(null);
  let summary = $state<string | null>(null);
  let isLoading = $state(false);
  let error = $state<string | null>(null);
  /** Phase 8.2 — related items resolved from node metadata. */
  let relatedItems = $state<PeekRelatedItem[]>([]);

  async function open(id: string, name?: string, nodeSummary?: string) {
    if (nodeId === id && content !== null) return; // already loaded

    nodeId = id;
    nodeName = name ?? null;
    summary = nodeSummary ?? null;
    isLoading = true;
    error = null;
    content = null;
    relatedItems = [];

    // Fetch content and node metadata in parallel.
    const [contentRes, nodeRes] = await Promise.all([
      sdk.workspaces.getContent(id),
      sdk.workspaces.get(id),
    ]);

    isLoading = false;

    if (contentRes.error) {
      error = contentRes.error.message;
      return;
    }
    content = contentRes.data.content;

    // Phase 8.2 — resolve related items from metadata.
    if (nodeRes.data?.metadata) {
      const meta = nodeRes.data.metadata as Record<string, unknown>;
      const relatedIds = [
        ...((meta["related_node_ids"] as string[] | undefined) ?? []),
        ...((meta["linked_file_ids"] as string[] | undefined) ?? []),
      ].filter((v): v is string => typeof v === "string" && v.length > 0);

      if (relatedIds.length > 0) {
        // Resolve names in parallel; skip any that fail.
        const resolved = await Promise.all(
          relatedIds.map(async (relId) => {
            const res = await sdk.workspaces.get(relId);
            if (!res.data) return null;
            const k = res.data.semantic_kind;
            return {
              id: relId,
              name: res.data.name,
              kind: (k === "folder" ? "folder" : k === "thread" ? "thread" : "document") as PeekRelatedItem["kind"],
            };
          })
        );
        relatedItems = resolved.filter((v): v is PeekRelatedItem => v !== null);
      }
    }
  }

  function close() {
    nodeId = null;
    content = null;
    nodeName = null;
    summary = null;
    isLoading = false;
    error = null;
    relatedItems = [];
  }

  return {
    get nodeId() { return nodeId; },
    get content() { return content; },
    get nodeName() { return nodeName; },
    get summary() { return summary; },
    get isLoading() { return isLoading; },
    get error() { return error; },
    get isOpen() { return nodeId !== null; },
    /** Phase 8.2 — related nodes resolved from metadata. */
    get relatedItems() { return relatedItems; },
    open,
    close,
  };
}

export type PeekStore = ReturnType<typeof createPeekStore>;
