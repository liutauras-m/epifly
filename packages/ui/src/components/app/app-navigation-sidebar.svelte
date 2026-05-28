<script lang="ts">
  import ArrowLeftIcon from "@lucide/svelte/icons/arrow-left";
  import ChevronsUpDownIcon from "@lucide/svelte/icons/chevrons-up-down";
  import FilePlusIcon from "@lucide/svelte/icons/file-plus";
  import FileSearchIcon from "@lucide/svelte/icons/file-search";
  import FileTextIcon from "@lucide/svelte/icons/file-text";
  import FolderPlusIcon from "@lucide/svelte/icons/folder-plus";
  import FolderTreeIcon from "@lucide/svelte/icons/folder-tree";
  import MessageSquareIcon from "@lucide/svelte/icons/message-square";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import SearchIcon from "@lucide/svelte/icons/search";
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import SquarePenIcon from "@lucide/svelte/icons/square-pen";
  import XIcon from "@lucide/svelte/icons/x";
  import * as Avatar from "../ui/avatar/index.js";
  import * as Button from "../ui/button/index.js";
  import * as DropdownMenu from "../ui/dropdown-menu/index.js";
  import * as Input from "../ui/input/index.js";
  import * as Sidebar from "../ui/sidebar/index.js";
  import * as Skeleton from "../ui/skeleton/index.js";
  import AppSidebar from "./app-sidebar.svelte";
  import WorkspaceTree from "../workspace/workspace-tree.svelte";
  import type { WorkspaceDraft, WorkspaceNode } from "../workspace/workspace-tree.svelte";

  type ThreadItem = {
    id: string;
    title?: string | null;
    last_active?: string | null;
  };

  type Props = {
    activePath?: string;
    /** Live workspace tree nodes from the workspaces store. */
    workspaceNodes?: WorkspaceNode[];
    workspaceLoading?: boolean;
    workspaceCreating?: boolean;
    workspaceError?: string | null;
    /** Live thread list from the threads store, sorted by recency. */
    threads?: ThreadItem[];
    threadsLoading?: boolean;
    activeThreadId?: string | null;
    selectedWorkspaceNodeId?: string | null;
    onNewChat?: () => void;
    onThreadSelect?: (threadId: string) => void;
    onWorkspaceNodeSelect?: (nodeId: string) => void;
    onWorkspaceNodeCreate?: (kind: "folder" | "document", name: string, parentId?: string | null) => unknown | Promise<unknown>;
    /** Backend search — if provided, results replace the local name filter. */
    onSearch?: (query: string) => Promise<WorkspaceNode[]>;
  };

  let {
    activePath = "/",
    workspaceNodes = [],
    workspaceLoading = false,
    workspaceCreating = false,
    workspaceError = null,
    threads = [],
    threadsLoading = false,
    activeThreadId = null,
    selectedWorkspaceNodeId = null,
    onNewChat,
    onThreadSelect,
    onWorkspaceNodeSelect,
    onWorkspaceNodeCreate,
    onSearch
  }: Props = $props();

  let searchOpen = $state(false);
  let searchQuery = $state("");
  let searchInputEl = $state<HTMLInputElement | null>(null);
  let draft = $state<WorkspaceDraft | null>(null);

  // Async search state — null means "not yet searched / use local filter".
  let backendSearchResults = $state<WorkspaceNode[] | null>(null);
  let isSearching = $state(false);
  let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

  /** Flatten workspace tree for search. */
  function flattenNodes(nodes: WorkspaceNode[]): WorkspaceNode[] {
    const out: WorkspaceNode[] = [];
    for (const n of nodes) {
      out.push(n);
      if (n.children?.length) out.push(...flattenNodes(n.children));
    }
    return out;
  }

  const flatNodes = $derived(flattenNodes(workspaceNodes));
  const selectedWorkspaceNode = $derived(
    selectedWorkspaceNodeId ? flatNodes.find((node) => node.id === selectedWorkspaceNodeId) : undefined
  );
  const createTargetParentId = $derived(
    selectedWorkspaceNode?.kind === "folder"
      ? selectedWorkspaceNode.id
      : selectedWorkspaceNode?.parentId ?? null
  );

  // When backend search is available and has results, prefer those.
  // Otherwise fall back to client-side name filter over the local tree.
  const filteredSearchItems = $derived(
    backendSearchResults !== null
      ? backendSearchResults
      : flatNodes.filter((n) =>
          n.name.toLowerCase().includes(searchQuery.trim().toLowerCase())
        )
  );

  // Trigger backend search when the query changes (debounced 300 ms).
  // This is an async side effect — not pure derivation — so $effect is correct.
  $effect(() => {
    const query = searchQuery;
    if (searchDebounceTimer !== null) clearTimeout(searchDebounceTimer);
    if (!query.trim() || !onSearch) {
      backendSearchResults = null;
      isSearching = false;
      return;
    }
    isSearching = true;
    searchDebounceTimer = setTimeout(async () => {
      const results = await onSearch(query);
      backendSearchResults = results;
      isSearching = false;
    }, 300);
    return () => {
      if (searchDebounceTimer !== null) clearTimeout(searchDebounceTimer);
    };
  });

  $effect(() => {
    if (searchOpen && searchInputEl) {
      searchInputEl.focus();
    }
  });

  function openSearch() {
    searchOpen = true;
  }

  function closeSearch() {
    searchOpen = false;
    searchQuery = "";
    backendSearchResults = null;
    isSearching = false;
  }

  function startDraft(kind: WorkspaceDraft["kind"]) {
    searchOpen = false;
    draft = {
      kind,
      parentId: createTargetParentId,
      name: kind === "folder" ? "Untitled folder" : "Untitled.md"
    };
  }

  async function commitDraft(name: string) {
    if (!draft) return;
    const result = await onWorkspaceNodeCreate?.(draft.kind, name, draft.parentId);
    if (result !== null) draft = null;
  }

  /** Display at most 15 recent threads in the sidebar. */
  const recentThreads = $derived(threads.slice(0, 15));
</script>

<AppSidebar>
  {#snippet header()}
    <Sidebar.Menu>
      <Sidebar.MenuItem>
        <DropdownMenu.DropdownMenu>
          <DropdownMenu.DropdownMenuTrigger>
            {#snippet child({ props })}
              <Sidebar.MenuButton
                {...props}
                size="lg"
                class="h-12 gap-3 px-2 data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
              >
                <span
                  class="flex size-9 shrink-0 items-center justify-center rounded-[14px] bg-sidebar-primary text-sidebar-primary-foreground"
                  aria-hidden="true"
                >
                  <FolderTreeIcon size={18} strokeWidth={1.75} />
                </span>
                <span class="grid min-w-0 flex-1 text-left leading-tight">
                  <span class="truncate text-sm font-semibold tracking-[-0.02em]">Epifly</span>
                  <span class="truncate text-xs text-sidebar-foreground/60">Product workspace</span>
                </span>
                <ChevronsUpDownIcon class="ml-auto size-4 shrink-0 text-sidebar-foreground/70" strokeWidth={1.75} aria-hidden="true" />
              </Sidebar.MenuButton>
            {/snippet}
          </DropdownMenu.DropdownMenuTrigger>
          <DropdownMenu.DropdownMenuContent side="right" align="start" class="w-(--bits-dropdown-menu-anchor-width) min-w-56">
            <DropdownMenu.DropdownMenuLabel>Workspaces</DropdownMenu.DropdownMenuLabel>
            <DropdownMenu.DropdownMenuItem>
              <FolderTreeIcon class="size-4" strokeWidth={1.75} aria-hidden="true" />
              Epifly
            </DropdownMenu.DropdownMenuItem>
            <DropdownMenu.DropdownMenuItem>
              <PlusIcon class="size-4" strokeWidth={1.75} aria-hidden="true" />
              New workspace
            </DropdownMenu.DropdownMenuItem>
          </DropdownMenu.DropdownMenuContent>
        </DropdownMenu.DropdownMenu>
      </Sidebar.MenuItem>
    </Sidebar.Menu>
  {/snippet}

  {#snippet content()}
    <!-- Workspace / files section -->
    <Sidebar.Group aria-label="Files" class="pt-1">
      <Sidebar.GroupContent>
        {#if searchOpen}
          <div class="app-sidebar-panel-swap space-y-2 group-data-[collapsible=icon]:hidden" aria-label="Search files panel">
            <div class="flex items-center gap-1 px-1">
              <Button.Button type="button" variant="ghost" size="icon-sm" aria-label="Back to file explorer" onclick={closeSearch} class="size-7 text-sidebar-foreground/70 hover:text-sidebar-foreground">
                <ArrowLeftIcon size={15} strokeWidth={1.75} aria-hidden="true" />
              </Button.Button>
              <div class="relative min-w-0 flex-1">
                <SearchIcon class="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-sidebar-foreground/45" strokeWidth={1.75} aria-hidden="true" />
                <Input.Input
                  bind:ref={searchInputEl}
                  bind:value={searchQuery}
                  type="search"
                  placeholder="Search files"
                  aria-label="Search files"
                  class="h-7 rounded-md border-sidebar-border bg-sidebar-accent/45 pl-7 pr-7 text-xs text-sidebar-foreground placeholder:text-sidebar-foreground/45 focus-visible:ring-sidebar-ring/35"
                  onkeydown={(event) => {
                    if (event.key === "Escape") {
                      event.preventDefault();
                      event.stopPropagation();
                      closeSearch();
                    }
                  }}
                />
                {#if searchQuery}
                  <Button.Button type="button" variant="ghost" size="icon-sm" aria-label="Clear search" onclick={() => (searchQuery = "")} class="absolute right-0.5 top-1/2 size-6 -translate-y-1/2 text-sidebar-foreground/55 hover:text-sidebar-foreground">
                    <XIcon size={13} strokeWidth={1.75} aria-hidden="true" />
                  </Button.Button>
                {/if}
              </div>
            </div>

            <Sidebar.Menu>
              {#if filteredSearchItems.length > 0}
                {#each filteredSearchItems as item (item.id)}
                  <Sidebar.MenuItem>
                    <Sidebar.MenuButton class="h-7 text-sidebar-foreground/80 hover:text-sidebar-foreground">
                      {#snippet child({ props })}
                        <button
                          type="button"
                          {...props}
                          onclick={() => { onWorkspaceNodeSelect?.(item.id); closeSearch(); }}
                        >
                          <FileTextIcon size={14} strokeWidth={1.75} aria-hidden="true" />
                          <span>{item.name}</span>
                        </button>
                      {/snippet}
                    </Sidebar.MenuButton>
                  </Sidebar.MenuItem>
                {/each}
              {:else}
                <Sidebar.MenuItem>
                  <Sidebar.MenuButton class="pointer-events-none h-8 text-sidebar-foreground/45">
                    <FileSearchIcon size={15} strokeWidth={1.75} aria-hidden="true" />
                    <span>
                      {#if isSearching}
                        Searching…
                      {:else if searchQuery}
                        No files found
                      {:else}
                        Start typing to search
                      {/if}
                    </span>
                  </Sidebar.MenuButton>
                </Sidebar.MenuItem>
              {/if}
            </Sidebar.Menu>
          </div>
        {:else}
          <div class="app-sidebar-panel-swap">
            <div class="mb-1 flex items-center gap-1 px-1 group-data-[collapsible=icon]:hidden" aria-label="File actions">
              <Button.Button type="button" variant="ghost" size="icon-sm" aria-label="Search files" onclick={openSearch} class="size-7 text-sidebar-foreground/70 hover:text-sidebar-foreground">
                <SearchIcon size={15} strokeWidth={1.75} aria-hidden="true" />
              </Button.Button>
              <Button.Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={createTargetParentId ? "New file in selected folder" : "New file"}
                disabled={workspaceCreating || !onWorkspaceNodeCreate}
                onclick={() => startDraft("document")}
                class="size-7 text-sidebar-foreground/70 hover:text-sidebar-foreground disabled:opacity-45"
              >
                <FilePlusIcon size={15} strokeWidth={1.75} aria-hidden="true" />
              </Button.Button>
              <Button.Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={createTargetParentId ? "New folder in selected folder" : "New folder"}
                disabled={workspaceCreating || !onWorkspaceNodeCreate}
                onclick={() => startDraft("folder")}
                class="size-7 text-sidebar-foreground/70 hover:text-sidebar-foreground disabled:opacity-45"
              >
                <FolderPlusIcon size={15} strokeWidth={1.75} aria-hidden="true" />
              </Button.Button>
            </div>

            {#if selectedWorkspaceNode && !searchOpen}
              <p class="mb-1 truncate px-2 text-[0.68rem] leading-5 text-sidebar-foreground/45 group-data-[collapsible=icon]:hidden">
                {selectedWorkspaceNode.kind === "folder" ? "Creating inside" : "Selected"}: {selectedWorkspaceNode.name}
              </p>
            {/if}

            {#if workspaceLoading}
              <div class="space-y-1 px-2 group-data-[collapsible=icon]:hidden" aria-label="Loading workspace">
                {#each [1, 2, 3] as i (i)}
                  <Skeleton.Skeleton class="h-6 w-full rounded-md" />
                {/each}
              </div>
            {:else if workspaceNodes.length > 0 || draft}
              <WorkspaceTree
                nodes={workspaceNodes}
                activeId={selectedWorkspaceNodeId ?? undefined}
                onSelect={onWorkspaceNodeSelect}
                {draft}
                onDraftCommit={commitDraft}
                onDraftCancel={() => (draft = null)}
                class="group-data-[collapsible=icon]:hidden"
              />
            {:else}
              <p class="px-3 py-2 text-xs text-sidebar-foreground/40 group-data-[collapsible=icon]:hidden">
                No files yet
              </p>
            {/if}

            {#if workspaceError}
              <p role="alert" class="px-3 py-1 text-xs text-destructive group-data-[collapsible=icon]:hidden">
                {workspaceError}
              </p>
            {/if}
          </div>
        {/if}
      </Sidebar.GroupContent>
    </Sidebar.Group>

    <!-- Chat history section -->
    <Sidebar.Group aria-label="Chat history" class="mt-2 pt-1">
      <Sidebar.GroupContent>
        <Sidebar.Menu>
          <!-- New chat button -->
          <Sidebar.MenuItem>
            <Sidebar.MenuButton tooltipContent="New chat">
              {#snippet child({ props })}
                <button type="button" {...props} onclick={onNewChat}>
                  <SquarePenIcon size={16} strokeWidth={1.75} aria-hidden="true" />
                  <span>New chat</span>
                </button>
              {/snippet}
            </Sidebar.MenuButton>
          </Sidebar.MenuItem>

          {#if threadsLoading && recentThreads.length === 0}
            {#each [1, 2, 3] as i (i)}
              <Sidebar.MenuItem>
                <Sidebar.MenuButton class="pointer-events-none">
                  <Skeleton.Skeleton class="h-4 w-4 rounded" />
                  <Skeleton.Skeleton class="h-3.5 flex-1 rounded" />
                </Sidebar.MenuButton>
              </Sidebar.MenuItem>
            {/each}
          {:else}
            {#each recentThreads as thread (thread.id)}
              {@const isActive = thread.id === activeThreadId}
              <Sidebar.MenuItem>
                <Sidebar.MenuButton
                  isActive={isActive}
                  tooltipContent={thread.title ?? "Untitled"}
                >
                  {#snippet child({ props })}
                    <button
                      type="button"
                      {...props}
                      onclick={() => onThreadSelect?.(thread.id)}
                      aria-current={isActive ? "page" : undefined}
                    >
                      <MessageSquareIcon size={16} strokeWidth={1.75} aria-hidden="true" />
                      <span class="truncate">{thread.title?.trim() || "Untitled"}</span>
                    </button>
                  {/snippet}
                </Sidebar.MenuButton>
              </Sidebar.MenuItem>
            {/each}
          {/if}
        </Sidebar.Menu>
      </Sidebar.GroupContent>
    </Sidebar.Group>
  {/snippet}

  {#snippet footer()}
    <Sidebar.Menu>
      <Sidebar.MenuItem>
        <DropdownMenu.DropdownMenu>
          <DropdownMenu.DropdownMenuTrigger>
            {#snippet child({ props })}
              <Sidebar.MenuButton
                {...props}
                size="lg"
                class="h-11 gap-3 px-2 data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
              >
                <Avatar.Avatar class="size-8">
                  <Avatar.AvatarFallback class="bg-sidebar-accent text-sm font-semibold text-sidebar-foreground">
                    LM
                  </Avatar.AvatarFallback>
                </Avatar.Avatar>
                <span class="grid min-w-0 flex-1 text-left leading-tight">
                  <span class="truncate text-[0.95rem] font-semibold tracking-[-0.02em]">Liutauras</span>
                  <span class="truncate text-xs text-sidebar-foreground/60">liutauras@example.com</span>
                </span>
                <ChevronsUpDownIcon class="ml-auto size-4 shrink-0 text-sidebar-foreground/70" strokeWidth={1.75} aria-hidden="true" />
              </Sidebar.MenuButton>
            {/snippet}
          </DropdownMenu.DropdownMenuTrigger>
          <DropdownMenu.DropdownMenuContent side="top" align="start" class="w-(--bits-dropdown-menu-anchor-width) min-w-56">
            <DropdownMenu.DropdownMenuLabel>Profile</DropdownMenu.DropdownMenuLabel>
            <DropdownMenu.DropdownMenuItem>
              <SettingsIcon class="size-4" strokeWidth={1.75} aria-hidden="true" />
              Account settings
            </DropdownMenu.DropdownMenuItem>
            <DropdownMenu.DropdownMenuItem>
              <MessageSquareIcon class="size-4" strokeWidth={1.75} aria-hidden="true" />
              Support
            </DropdownMenu.DropdownMenuItem>
          </DropdownMenu.DropdownMenuContent>
        </DropdownMenu.DropdownMenu>
      </Sidebar.MenuItem>
    </Sidebar.Menu>
  {/snippet}
</AppSidebar>
