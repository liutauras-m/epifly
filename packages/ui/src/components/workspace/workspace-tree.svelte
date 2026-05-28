<script lang="ts">
  import { cn } from "../../utils/cn.js";
  import WorkspaceNodeRow from "./workspace-node-row.svelte";

  export type WorkspaceNode = {
    id: string;
    name: string;
    parentId?: string | null;
    kind: "folder" | "thread" | "document";
    children?: WorkspaceNode[];
    /** For kind === "thread": the originating thread_id. */
    threadId?: string | null;
    /** Full virtual path, e.g. "Clients/Acme/Kickoff". */
    virtualPath?: string;
    /** Work-unit status sourced from metadata. */
    status?: "active" | "paused" | "done" | "archived";
    summary?: string;
    lastActivityAt?: string;
    tags?: string[];
  };

  export type WorkspaceDraft = {
    kind: "folder" | "document";
    parentId: string | null;
    name: string;
  };

  type Props = {
    nodes: WorkspaceNode[];
    activeId?: string;
    onSelect?: (id: string) => void;
    /** Called when a thread row is clicked; receives the threadId. */
    onOpenThread?: (threadId: string) => void;
    /** Called when a DnD drop or "Move to" action fires; receives (sourceId, targetFolderId). */
    onMove?: (sourceId: string, targetId: string | null) => void;
    /** Called when a rename is committed; receives (nodeId, newName). */
    onRename?: (nodeId: string, newName: string) => void;
    draft?: WorkspaceDraft | null;
    onDraftCommit?: (name: string) => void | Promise<void>;
    onDraftCancel?: () => void;
    class?: string;
  };

  let { nodes, activeId, onSelect, onOpenThread, onMove, onRename, draft = null, onDraftCommit, onDraftCancel, class: className }: Props = $props();
</script>

<nav class={cn("flex flex-col gap-0.5 py-2", className)} aria-label="Workspace">
  {#if draft && draft.parentId === null}
    <WorkspaceNodeRow draft={draft} {activeId} onDraftCommit={onDraftCommit} onDraftCancel={onDraftCancel} depth={0} />
  {/if}
  {#each nodes as node (node.id)}
    <WorkspaceNodeRow {node} {activeId} onSelect={onSelect} {onOpenThread} {onMove} {onRename} {draft} onDraftCommit={onDraftCommit} onDraftCancel={onDraftCancel} depth={0} />
  {/each}
</nav>
