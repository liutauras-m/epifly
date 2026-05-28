/**
 * Adapters from backend WorkspaceNode shapes to the UI sidebar shape.
 *
 * The output type mirrors WorkspaceNode from @epifly/ui/workspace-tree.svelte.
 * Defined locally to keep @epifly/features independent of @epifly/ui.
 * TypeScript structural typing ensures compatibility at call sites.
 *
 * INVARIANT: Branch on `semantic_kind`, never on storage `kind` or mime_type.
 * `kind` is a storage hint (folder/conversation/file/artifact); `semantic_kind`
 * is what the UX must branch on (folder/thread/file).
 */

/** Semantic kind of a sidebar node — what the UX must use for branching. */
export type SidebarNodeKind = "folder" | "thread" | "document";

/**
 * Work-unit status, sourced from `WorkspaceNode.metadata.status`.
 * Promoted to a typed field once stable enough.
 */
export type WorkUnitStatus = "active" | "paused" | "done" | "archived";

export type SidebarWorkspaceNode = {
  id: string;
  name: string;
  kind: SidebarNodeKind;
  parentId?: string | null;
  children?: SidebarWorkspaceNode[];
  /** Thread identifier; set only when kind === "thread" (from WorkspaceNode.source_id). */
  threadId?: string | null;
  /** Full virtual path, e.g. "Clients/Acme/Kickoff". Present for all nodes. */
  virtualPath?: string;
  /**
   * Work-unit fields — sourced from WorkspaceNode.metadata or top-level columns.
   * No UI behaviour yet; used by Smart Views (Phase 2) and Memory (Phase 8).
   * New fields land in metadata first; promote to columns only once proven stable.
   */
  status?: WorkUnitStatus;
  summary?: string;
  lastActivityAt?: string;
  tags: string[];
  relatedNodeIds?: string[];
};

/**
 * Minimal backend shape this adapter reads from.
 * Deliberately narrow — we only require what we actually use.
 * `WorkspaceNode` from @conusai/types satisfies this.
 */
type WorkspaceNodeLike = {
  id: string;
  name: string;
  parent_id?: string | null;
  /** Storage/mime hint. Do NOT use for UX branching. */
  kind: string;
  /** Semantic kind — the single field that drives UX branching. */
  semantic_kind: string;
  virtual_path?: string;
  last_modified?: string;
  source_id?: string | null;
  tags?: string[];
  /**
   * Metadata bag.
   * Sub-schema (all optional, all best-effort):
   *   status       : "active" | "paused" | "done" | "archived"
   *   summary      : string   — short human-readable summary
   *   relatedNodeIds: string[] — node IDs related via memory/entity extraction
   *   thread_id    : string   — redundant with source_id; kept for legacy compat
   */
  metadata?: Record<string, unknown> | null;
  children?: WorkspaceNodeLike[];
};

export function toSidebarWorkspaceNode(node: WorkspaceNodeLike): SidebarWorkspaceNode {
  const kind = toSidebarNodeKind(node.semantic_kind);
  const meta = node.metadata ?? {};

  return {
    id: node.id,
    name: node.name,
    parentId: node.parent_id ?? null,
    kind,
    virtualPath: node.virtual_path ?? "",
    // Thread identity — only meaningful when kind === "thread"
    threadId: kind === "thread" ? (node.source_id ?? null) : undefined,
    // Work-unit fields — sourced from metadata; default to safe empty values
    status: isWorkUnitStatus(meta.status) ? meta.status : undefined,
    summary: typeof meta.summary === "string" ? meta.summary : undefined,
    lastActivityAt: node.last_modified,
    tags: Array.isArray(node.tags) ? node.tags : [],
    relatedNodeIds: Array.isArray(meta.relatedNodeIds)
      ? (meta.relatedNodeIds as string[])
      : undefined,
    children: node.children?.map((child) => toSidebarWorkspaceNode(child)),
  };
}

/** Maps semantic_kind → sidebar kind. This is the single translation point. */
function toSidebarNodeKind(semanticKind: string): SidebarNodeKind {
  switch (semanticKind) {
    case "thread":  return "thread";
    case "folder":  return "folder";
    default:        return "document";
  }
}

const VALID_STATUSES = new Set(["active", "paused", "done", "archived"]);
function isWorkUnitStatus(value: unknown): value is WorkUnitStatus {
  return typeof value === "string" && VALID_STATUSES.has(value);
}
