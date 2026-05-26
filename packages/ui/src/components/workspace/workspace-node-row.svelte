<script lang="ts">
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import FileTextIcon from "@lucide/svelte/icons/file-text";
  import MessageSquareIcon from "@lucide/svelte/icons/message-square";
  import { cn } from "../../utils/cn.js";
  import * as Button from "../ui/button/index.js";
  import type { WorkspaceNode } from "./workspace-tree.svelte";
  import WorkspaceNodeRow from "./workspace-node-row.svelte";

  type Props = {
    node: WorkspaceNode;
    activeId?: string;
    onSelect?: (id: string) => void;
    depth?: number;
  };

  let { node, activeId, onSelect, depth = 0 }: Props = $props();

  let expanded = $state(true);
  let isActive = $derived(node.id === activeId);
  let hasChildren = $derived(node.kind === "folder" && !!node.children?.length);
</script>

<div>
  <Button.Button
    type="button"
    variant="ghost"
    onclick={() => {
      if (node.kind === "folder") {
        expanded = !expanded;
      }
      onSelect?.(node.id);
    }}
    class={cn(
      "h-auto w-full justify-start gap-2 px-2 py-1.5 text-sm font-normal",
      isActive && "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
    )}
    style="padding-left: {0.5 + depth * 1}rem"
    aria-expanded={node.kind === "folder" ? expanded : undefined}
  >
    {#if node.kind === "folder"}
      <ChevronRightIcon class={cn("size-3.5 shrink-0 transition-transform", expanded && "rotate-90")} strokeWidth={1.75} aria-hidden="true" />
    {:else if node.kind === "thread"}
      <MessageSquareIcon class="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.75} aria-hidden="true" />
    {:else}
      <FileTextIcon class="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.75} aria-hidden="true" />
    {/if}

    <span class="flex-1 truncate text-left">{node.name}</span>
  </Button.Button>

  {#if hasChildren && expanded}
    <div>
      {#each (node.children ?? []) as child (child.id)}
        <WorkspaceNodeRow node={child} {activeId} onSelect={onSelect} depth={depth + 1} />
      {/each}
    </div>
  {/if}
</div>
