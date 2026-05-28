<script lang="ts">
  /**
   * Smart Views lane — filters, not folders.
   *
   * Renders a labelled list of named views. When a view is active it shows
   * a flat result list (same WorkspaceNode row component + identity).
   *
   * Design intent:
   * - Lane sits BELOW the tree in the sidebar (after Place, before Search/Cmd+K).
   * - View list is collapsed when a view is inactive; the active view expands inline.
   * - Results reuse WorkspaceNodeRow for consistent affordances + theming.
   */

  import ClockIcon from "@lucide/svelte/icons/clock";
  import FilterIcon from "@lucide/svelte/icons/filter";
  import InboxIcon from "@lucide/svelte/icons/inbox";
  import PauseIcon from "@lucide/svelte/icons/pause";
  import XIcon from "@lucide/svelte/icons/x";
  import * as Sidebar from "../ui/sidebar/index.js";
  import * as Skeleton from "../ui/skeleton/index.js";
  import * as Button from "../ui/button/index.js";
  import WorkspaceNodeRow from "./workspace-node-row.svelte";
  import type { WorkspaceNode } from "./workspace-tree.svelte";

  export type SmartViewKind = "unsorted" | "recently-updated" | "paused";

  export type SmartViewDef = {
    kind: SmartViewKind;
    label: string;
    description: string;
  };

  const VIEWS: SmartViewDef[] = [
    {
      kind: "unsorted",
      label: "Unsorted",
      description: "Conversations not yet filed into a folder",
    },
    {
      kind: "recently-updated",
      label: "Recently updated",
      description: "All items sorted by last activity",
    },
    {
      kind: "paused",
      label: "Paused",
      description: "Conversations you paused — restore to continue",
    },
  ];

  type Props = {
    activeView?: SmartViewKind | null;
    results?: WorkspaceNode[];
    isLoading?: boolean;
    error?: string | null;
    activeNodeId?: string | null;
    onSelectView?: (kind: SmartViewKind) => void;
    onClearView?: () => void;
    onSelectNode?: (id: string) => void;
    onOpenThread?: (threadId: string) => void;
    /** Called when the user restores a paused thread from the Paused view. */
    onRestoreThread?: (threadId: string) => void;
  };

  let {
    activeView = null,
    results = [],
    isLoading = false,
    error = null,
    activeNodeId = null,
    onSelectView,
    onClearView,
    onSelectNode,
    onOpenThread,
    onRestoreThread,
  }: Props = $props();

  const activeViewDef = $derived(VIEWS.find((v) => v.kind === activeView) ?? null);
</script>

<Sidebar.Group aria-label="Smart Views" class="mt-1 pt-1">
  <div class="flex items-center gap-1 px-2 pb-1">
    <FilterIcon class="size-3 shrink-0 text-sidebar-foreground/45" strokeWidth={1.75} aria-hidden="true" />
    <span class="text-[0.68rem] font-medium uppercase tracking-wide text-sidebar-foreground/45">
      Smart Views
    </span>
    {#if activeView}
      <Button.Button
        type="button"
        variant="ghost"
        size="icon-sm"
        class="ml-auto size-5 text-sidebar-foreground/50 hover:text-sidebar-foreground"
        aria-label="Close {activeViewDef?.label ?? 'view'}"
        onclick={onClearView}
      >
        <XIcon size={12} strokeWidth={2} aria-hidden="true" />
      </Button.Button>
    {/if}
  </div>

  <Sidebar.GroupContent>
    {#if !activeView}
      <!-- View picker -->
      <Sidebar.Menu>
        {#each VIEWS as view (view.kind)}
          <Sidebar.MenuItem>
            <Sidebar.MenuButton tooltipContent={view.description}>
              {#snippet child({ props })}
                <button
                  type="button"
                  {...props}
                  class="{props.class} h-7 text-sidebar-foreground/70 hover:text-sidebar-foreground"
                  onclick={() => onSelectView?.(view.kind)}
                  aria-label={view.description}
                >
                  {#if view.kind === "unsorted"}
                    <InboxIcon size={14} strokeWidth={1.75} aria-hidden="true" />
                  {:else if view.kind === "paused"}
                    <PauseIcon size={14} strokeWidth={1.75} aria-hidden="true" />
                  {:else}
                    <ClockIcon size={14} strokeWidth={1.75} aria-hidden="true" />
                  {/if}
                  <span class="truncate">{view.label}</span>
                </button>
              {/snippet}
            </Sidebar.MenuButton>
          </Sidebar.MenuItem>
        {/each}
      </Sidebar.Menu>
    {:else}
      <!-- Active view results -->
      <div class="mb-1 px-2 text-[0.68rem] font-medium text-sidebar-foreground/60">
        {activeViewDef?.label ?? activeView}
      </div>

      {#if isLoading}
        <div class="space-y-1 px-2" aria-label="Loading {activeViewDef?.label ?? 'view'}">
          {#each [1, 2, 3] as i (i)}
            <Skeleton.Skeleton class="h-6 w-full rounded-md" />
          {/each}
        </div>
      {:else if error}
        <p role="alert" class="px-3 py-1 text-xs text-destructive">{error}</p>
      {:else if results.length === 0}
        <p class="px-3 py-2 text-xs text-sidebar-foreground/40">
          {#if activeView === "unsorted"}
            No unsorted conversations
          {:else if activeView === "paused"}
            No paused conversations
          {:else}
            Nothing here yet
          {/if}
        </p>
      {:else}
        <nav class="flex flex-col gap-0.5 py-1" aria-label="{activeViewDef?.label ?? 'Smart view'} results">
          {#each results as node (node.id)}
            <WorkspaceNodeRow
              {node}
              activeId={activeNodeId ?? undefined}
              onSelect={onSelectNode}
              {onOpenThread}
              onRestore={activeView === "paused" ? onRestoreThread : undefined}
              depth={0}
            />
          {/each}
        </nav>
      {/if}
    {/if}
  </Sidebar.GroupContent>
</Sidebar.Group>
