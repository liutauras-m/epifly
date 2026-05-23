<script lang="ts">
  import type { WorkspaceNode } from "@conusai/types";

  interface Props {
    nodes: WorkspaceNode[];
    selectedId?: string;
    onselect?: (node: WorkspaceNode) => void;
  }

  let { nodes, selectedId, onselect }: Props = $props();

  function icon(kind: WorkspaceNode["kind"]) {
    return kind === "folder" ? "📁" : kind === "file" ? "📄" : "🔷";
  }
</script>

<nav aria-label="Workspace tree">
  <ul class="tree" role="tree">
    {#each nodes as node (node.id)}
      <li
        role="treeitem"
        aria-selected={node.id === selectedId}
        class="node"
        class:selected={node.id === selectedId}
        tabindex="0"
        onclick={() => onselect?.(node)}
        onkeydown={(e) => e.key === "Enter" && onselect?.(node)}
      >
        <span class="icon" aria-hidden="true">{icon(node.kind)}</span>
        <span class="label">{node.name}</span>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .tree {
    list-style: none;
    padding: var(--space-2) 0;
    margin: 0;
  }

  .node {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-3);
    cursor: pointer;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-meta);
    color: var(--color-fg-muted);
    transition: background var(--duration-fast) var(--ease-standard);  /* [feedback] */
    min-height: var(--hit, 44px);
  }

  .node:hover { background: var(--color-bg-hover); color: var(--color-fg); }
  .node:focus-visible {
    outline: var(--focus-ring);
    outline-offset: var(--focus-ring-offset);
  }
  .node.selected { background: var(--color-accent-soft); color: var(--color-fg); }

  .icon { flex-shrink: 0; font-size: 14px; }
  .label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
