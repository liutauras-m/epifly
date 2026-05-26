<script lang="ts">
  import { cn } from "../../utils/cn.js";
  import WorkspaceNodeRow from "./workspace-node-row.svelte";

  export type WorkspaceNode = {
    id: string;
    name: string;
    kind: "folder" | "thread" | "document";
    children?: WorkspaceNode[];
  };

  type Props = {
    nodes: WorkspaceNode[];
    activeId?: string;
    onselect?: (id: string) => void;
    class?: string;
  };

  let { nodes, activeId, onselect, class: className }: Props = $props();
</script>

<nav class={cn("flex flex-col gap-0.5 py-2", className)} aria-label="Workspace">
  {#each nodes as node (node.id)}
    <WorkspaceNodeRow {node} {activeId} {onselect} depth={0} />
  {/each}
</nav>
