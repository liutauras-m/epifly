<script lang="ts">
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import FileTextIcon from "@lucide/svelte/icons/file-text";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import MessageSquareIcon from "@lucide/svelte/icons/message-square";
  import MoreHorizontalIcon from "@lucide/svelte/icons/more-horizontal";
  import { cn } from "../../utils/cn.js";
  import type { WorkspaceDraft, WorkspaceNode } from "./workspace-tree.svelte";
  import WorkspaceNodeRow from "./workspace-node-row.svelte";
  import * as DropdownMenu from "../ui/dropdown-menu/index.js";
  import * as Button from "../ui/button/index.js";

  type Props = {
    node?: WorkspaceNode;
    activeId?: string;
    onSelect?: (id: string) => void;
    /** Called when a kind:"thread" row is clicked. Receives the threadId. */
    onOpenThread?: (threadId: string) => void;
    /**
     * Called when a node is dropped onto a folder target.
     * Step 3.1: receives (sourceNodeId, targetFolderId).
     */
    onMove?: (sourceId: string, targetId: string | null) => void;
    /**
     * Called when the user commits a rename (inline input or F2).
     * Step 3.2: receives (nodeId, newName).
     */
    onRename?: (nodeId: string, newName: string) => void;
    depth?: number;
    draft?: WorkspaceDraft | null;
    onDraftCommit?: (name: string) => void | Promise<void>;
    onDraftCancel?: () => void;
  };

  let {
    node,
    activeId,
    onSelect,
    onOpenThread,
    onMove,
    onRename,
    depth = 0,
    draft = null,
    onDraftCommit,
    onDraftCancel
  }: Props = $props();

  let expanded = $state(false);
  let draftInputEl = $state<HTMLInputElement | null>(null);
  let draftName = $state("");
  let renameInputEl = $state<HTMLInputElement | null>(null);
  let isRenaming = $state(false);
  let renameDraft = $state("");
  let isDragOver = $state(false);

  let isDraft = $derived(!node && !!draft);
  let isActive = $derived(!!node && node.id === activeId);
  let hasChildren = $derived(!!node && node.kind === "folder" && !!node.children?.length);
  let isDraftTarget = $derived(!!node && !!draft && draft.parentId === node.id);

  $effect(() => {
    if (isDraftTarget) expanded = true;
  });

  $effect(() => {
    if (isDraft && draft) {
      draftName = draft.name;
      requestAnimationFrame(() => {
        draftInputEl?.focus();
        draftInputEl?.select();
      });
    }
  });

  $effect(() => {
    if (isRenaming) {
      requestAnimationFrame(() => {
        renameInputEl?.focus();
        renameInputEl?.select();
      });
    }
  });

  async function commitDraft() {
    const trimmed = draftName.trim();
    if (!trimmed) {
      onDraftCancel?.();
      return;
    }
    await onDraftCommit?.(trimmed);
  }

  function handleDraftKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      void commitDraft();
    }
    if (event.key === "Escape") {
      event.preventDefault();
      onDraftCancel?.();
    }
  }

  function startRename() {
    if (!node || node.kind === "folder" && !onRename) return;
    renameDraft = node.name;
    isRenaming = true;
  }

  function commitRename() {
    const trimmed = renameDraft.trim();
    isRenaming = false;
    if (trimmed && trimmed !== node?.name && node) {
      onRename?.(node.id, trimmed);
    }
  }

  function handleRenameKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      commitRename();
    }
    if (event.key === "Escape") {
      event.preventDefault();
      isRenaming = false;
    }
  }

  // ── Drag-and-drop (Step 3.1) ──────────────────────────────────────────────

  function handleDragStart(event: DragEvent) {
    if (!node || !event.dataTransfer) return;
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", node.id);
  }

  function handleDragOver(event: DragEvent) {
    if (!node || node.kind !== "folder") return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    isDragOver = true;
  }

  function handleDragLeave() {
    isDragOver = false;
  }

  function handleDrop(event: DragEvent) {
    isDragOver = false;
    if (!node || node.kind !== "folder" || !event.dataTransfer) return;
    event.preventDefault();
    const sourceId = event.dataTransfer.getData("text/plain");
    if (sourceId && sourceId !== node.id) {
      onMove?.(sourceId, node.id);
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!node) return;
    // F2 → rename
    if (event.key === "F2" && onRename) {
      event.preventDefault();
      startRename();
    }
  }
</script>

<div>
  {#if isDraft && draft}
    <div
      class="flex min-h-7 items-center gap-2 rounded-md bg-sidebar-accent/75 px-2 py-1 text-sm text-sidebar-accent-foreground ring-1 ring-sidebar-ring/25"
      style="padding-left: {0.5 + depth * 1}rem"
      role="treeitem"
      aria-selected="true"
      aria-label={draft.kind === "folder" ? "Name new folder" : "Name new file"}
    >
      {#if draft.kind === "folder"}
        <FolderIcon class="size-3.5 shrink-0 text-sidebar-foreground/70" strokeWidth={1.75} aria-hidden="true" />
      {:else}
        <FileTextIcon class="size-3.5 shrink-0 text-sidebar-foreground/70" strokeWidth={1.75} aria-hidden="true" />
      {/if}
      <input
        bind:this={draftInputEl}
        bind:value={draftName}
        class="h-6 min-w-0 flex-1 rounded-[6px] border border-sidebar-ring/35 bg-background/95 px-2 text-xs text-foreground shadow-sm outline-none focus:border-sidebar-ring focus:ring-2 focus:ring-sidebar-ring/20"
        aria-label={draft.kind === "folder" ? "New folder name" : "New file name"}
        onkeydown={handleDraftKeydown}
        onblur={() => void commitDraft()}
      />
    </div>
  {:else if node}
    <!-- Row container — also the drag-drop target for folders -->
    <div
      class={cn(
        "group relative flex min-h-7 items-center gap-1 rounded-md text-sm text-sidebar-foreground/78 transition-colors duration-[var(--motion-fast)] ease-[var(--ease-standard)] hover:bg-sidebar-accent/60 hover:text-sidebar-foreground",
        isActive && "bg-sidebar-accent text-sidebar-accent-foreground ring-1 ring-sidebar-ring/20",
        isDragOver && "ring-2 ring-sidebar-ring/60 bg-sidebar-accent/40"
      )}
      style="padding-left: {0.25 + depth * 1}rem"
      role="treeitem"
      tabindex="-1"
      aria-selected={isActive}
      aria-expanded={node.kind === "folder" ? expanded : undefined}
      draggable="true"
      ondragstart={handleDragStart}
      ondragover={handleDragOver}
      ondragleave={handleDragLeave}
      ondrop={handleDrop}
      onkeydown={handleKeydown}
    >
      {#if node.kind === "folder"}
        <button
          type="button"
          class="flex size-6 shrink-0 items-center justify-center rounded-[6px] text-sidebar-foreground/60 outline-none transition-colors hover:bg-sidebar-accent hover:text-sidebar-foreground focus-visible:ring-2 focus-visible:ring-sidebar-ring/40"
          aria-label={expanded ? `Collapse ${node.name}` : `Expand ${node.name}`}
          onclick={(event) => {
            event.stopPropagation();
            const next = !expanded;
            expanded = next;
            // Load children on first expand (children===undefined means not fetched yet)
            if (next && node.children === undefined) onSelect?.(node.id);
          }}
        >
          <ChevronRightIcon class={cn("size-3.5 transition-transform duration-[var(--motion-fast)] ease-[var(--ease-standard)]", expanded && "rotate-90")} strokeWidth={1.75} aria-hidden="true" />
        </button>
      {:else}
        <span class="flex size-6 shrink-0 items-center justify-center" aria-hidden="true"></span>
      {/if}

      {#if isRenaming}
        <!-- Inline rename input (Step 3.2) -->
        <input
          bind:this={renameInputEl}
          bind:value={renameDraft}
          class="h-6 min-w-0 flex-1 rounded-[6px] border border-sidebar-ring/35 bg-background/95 px-2 text-xs text-foreground shadow-sm outline-none focus:border-sidebar-ring focus:ring-2 focus:ring-sidebar-ring/20"
          aria-label="Rename {node.name}"
          onkeydown={handleRenameKeydown}
          onblur={commitRename}
        />
      {:else}
        <button
          type="button"
          class="flex min-w-0 flex-1 items-center gap-2 rounded-[6px] py-1 pr-2 text-left outline-none focus-visible:ring-2 focus-visible:ring-sidebar-ring/40"
          onclick={() => {
            if (node.kind === "folder") {
              expanded = true;
              // Only load children if not yet fetched
              if (node.children === undefined) onSelect?.(node.id);
            } else if (node.kind === "thread" && node.threadId) {
              // Thread rows navigate to the conversation, never just "select".
              onOpenThread?.(node.threadId);
            } else {
              onSelect?.(node.id);
            }
          }}
          ondblclick={() => { if (onRename) startRename(); }}
          aria-current={isActive ? "true" : undefined}
        >
          {#if node.kind === "folder"}
            <FolderIcon class="size-3.5 shrink-0 text-sidebar-foreground/70" strokeWidth={1.75} aria-hidden="true" />
          {:else if node.kind === "thread"}
            <MessageSquareIcon class="size-3.5 shrink-0 text-sidebar-foreground/60" strokeWidth={1.75} aria-hidden="true" />
          {:else}
            <FileTextIcon class="size-3.5 shrink-0 text-sidebar-foreground/60" strokeWidth={1.75} aria-hidden="true" />
          {/if}
          <span class="flex-1 truncate">{node.name}</span>
        </button>

        <!-- Context menu (hover/focus-visible only) — keyboard + mouse accessible -->
        {#if onMove || onRename}
          <DropdownMenu.DropdownMenu>
            <DropdownMenu.DropdownMenuTrigger>
              {#snippet child({ props })}
                <Button.Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  {...props}
                  class="invisible size-5 shrink-0 text-sidebar-foreground/40 opacity-0 transition-opacity group-hover:visible group-hover:opacity-100 focus:visible focus:opacity-100 focus-visible:visible focus-visible:opacity-100"
                  aria-label="More actions for {node.name}"
                  onclick={(e: MouseEvent) => e.stopPropagation()}
                >
                  <MoreHorizontalIcon size={12} strokeWidth={2} aria-hidden="true" />
                </Button.Button>
              {/snippet}
            </DropdownMenu.DropdownMenuTrigger>
            <DropdownMenu.DropdownMenuContent align="start" class="w-36">
              {#if onRename}
                <DropdownMenu.DropdownMenuItem onclick={() => startRename()}>
                  Rename
                </DropdownMenu.DropdownMenuItem>
              {/if}
              {#if onMove}
                <DropdownMenu.DropdownMenuItem onclick={() => onMove!(node.id, null)}>
                  Move to root
                </DropdownMenu.DropdownMenuItem>
              {/if}
            </DropdownMenu.DropdownMenuContent>
          </DropdownMenu.DropdownMenu>
        {/if}
      {/if}
    </div>

    {#if node.kind === "folder" && expanded}
      <div role="group" class="mt-0.5 flex flex-col gap-0.5">
        {#if draft && draft.parentId === node.id}
          <WorkspaceNodeRow {draft} {activeId} onDraftCommit={onDraftCommit} onDraftCancel={onDraftCancel} depth={depth + 1} />
        {/if}
        {#each (node.children ?? []) as child (child.id)}
          <WorkspaceNodeRow node={child} {activeId} onSelect={onSelect} {onOpenThread} {onMove} {onRename} {draft} onDraftCommit={onDraftCommit} onDraftCancel={onDraftCancel} depth={depth + 1} />
        {/each}
      </div>
    {/if}
  {/if}
</div>
