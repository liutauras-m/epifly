<script lang="ts">
  import ArrowLeftIcon from "@lucide/svelte/icons/arrow-left";
  import ChevronsUpDownIcon from "@lucide/svelte/icons/chevrons-up-down";
  import FilePlusIcon from "@lucide/svelte/icons/file-plus";
  import FileSearchIcon from "@lucide/svelte/icons/file-search";
  import FileTextIcon from "@lucide/svelte/icons/file-text";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import FolderPlusIcon from "@lucide/svelte/icons/folder-plus";
  import FolderOpenIcon from "@lucide/svelte/icons/folder-open";
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
  import AppSidebar from "./app-sidebar.svelte";

  type Props = {
    activePath?: string;
  };

  let { activePath = "/" }: Props = $props();

  const fileBranches = [
    {
      title: "apps",
      children: ["web", "native"]
    },
    {
      title: "packages",
      children: ["ui", "sdk"]
    },
    {
      title: "docs",
      children: ["plan.md"]
    }
  ];

  let openBranches = $state(new Set(fileBranches.map((branch) => branch.title)));
  let searchOpen = $state(false);
  let searchQuery = $state("");
  let searchInputEl = $state<HTMLInputElement | null>(null);

  const searchItems = $derived(
    fileBranches.flatMap((branch) => [
      {
        name: branch.title,
        path: branch.title,
        type: "folder" as const
      },
      ...branch.children.map((file) => ({
        name: file,
        path: `${branch.title}/${file}`,
        type: "file" as const
      }))
    ])
  );

  const filteredSearchItems = $derived(
    searchItems.filter((item) =>
      `${item.name} ${item.path}`.toLowerCase().includes(searchQuery.trim().toLowerCase())
    )
  );

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
  }

  function toggleBranch(title: string) {
    const next = new Set(openBranches);
    if (next.has(title)) {
      next.delete(title);
    } else {
      next.add(title);
    }
    openBranches = next;
  }
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
                    if (event.key === "Escape") closeSearch();
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
              {#each filteredSearchItems as item (item.path)}
                <Sidebar.MenuItem>
                  <Sidebar.MenuButton class="h-7 text-sidebar-foreground/80 hover:text-sidebar-foreground">
                    {#snippet child({ props })}
                      <button type="button" {...props}>
                        {#if item.type === "folder"}
                          <FolderIcon size={15} strokeWidth={1.75} aria-hidden="true" />
                        {:else}
                          <FileTextIcon size={14} strokeWidth={1.75} aria-hidden="true" />
                        {/if}
                        <span>{item.name}</span>
                        <span class="ml-auto truncate text-[0.68rem] text-sidebar-foreground/40">{item.path}</span>
                      </button>
                    {/snippet}
                  </Sidebar.MenuButton>
                </Sidebar.MenuItem>
              {:else}
                <Sidebar.MenuItem>
                  <Sidebar.MenuButton class="pointer-events-none h-8 text-sidebar-foreground/45">
                    <FileSearchIcon size={15} strokeWidth={1.75} aria-hidden="true" />
                    <span>No files found</span>
                  </Sidebar.MenuButton>
                </Sidebar.MenuItem>
              {/each}
            </Sidebar.Menu>
          </div>
        {:else}
          <div class="app-sidebar-panel-swap">
            <div class="mb-1 flex items-center gap-1 px-1 group-data-[collapsible=icon]:hidden" aria-label="File actions">
              <Button.Button type="button" variant="ghost" size="icon-sm" aria-label="Search files" onclick={openSearch} class="size-7 text-sidebar-foreground/70 hover:text-sidebar-foreground">
                <SearchIcon size={15} strokeWidth={1.75} aria-hidden="true" />
              </Button.Button>
              <Button.Button type="button" variant="ghost" size="icon-sm" aria-label="New file" class="size-7 text-sidebar-foreground/70 hover:text-sidebar-foreground">
                <FilePlusIcon size={15} strokeWidth={1.75} aria-hidden="true" />
              </Button.Button>
              <Button.Button type="button" variant="ghost" size="icon-sm" aria-label="New folder" class="size-7 text-sidebar-foreground/70 hover:text-sidebar-foreground">
                <FolderPlusIcon size={15} strokeWidth={1.75} aria-hidden="true" />
              </Button.Button>
            </div>

            <Sidebar.Menu>
              {#each fileBranches as branch (branch.title)}
                {@const isOpen = openBranches.has(branch.title)}
                <Sidebar.MenuItem>
                  <Sidebar.MenuButton class="h-7 font-medium text-sidebar-foreground/90">
                    {#snippet child({ props })}
                      <button type="button" {...props} onclick={() => toggleBranch(branch.title)} aria-expanded={isOpen}>
                        {#if isOpen}
                          <FolderOpenIcon size={16} strokeWidth={1.75} aria-hidden="true" />
                        {:else}
                          <FolderIcon size={16} strokeWidth={1.75} aria-hidden="true" />
                        {/if}
                        <span>{branch.title}</span>
                      </button>
                    {/snippet}
                  </Sidebar.MenuButton>
                  <Sidebar.MenuSub class={isOpen ? "py-0.5" : "hidden"} aria-hidden={!isOpen}>
                    {#each branch.children as file (file)}
                      <Sidebar.MenuSubItem>
                        <Sidebar.MenuSubButton size="sm" class="h-6 text-sidebar-foreground/60 hover:text-sidebar-foreground">
                          {#snippet child({ props })}
                            <button type="button" {...props}>
                              <FileTextIcon size={14} strokeWidth={1.75} aria-hidden="true" />
                              <span>{file}</span>
                            </button>
                          {/snippet}
                        </Sidebar.MenuSubButton>
                      </Sidebar.MenuSubItem>
                    {/each}
                  </Sidebar.MenuSub>
                </Sidebar.MenuItem>
              {/each}
            </Sidebar.Menu>
          </div>
        {/if}
      </Sidebar.GroupContent>
    </Sidebar.Group>

    <Sidebar.Group aria-label="Chat history" class="mt-2 pt-1">
      <Sidebar.GroupContent>
        <Sidebar.Menu>
          <Sidebar.MenuItem>
            <Sidebar.MenuButton class="pointer-events-none opacity-70" tooltipContent="New chat">
              <SquarePenIcon size={16} strokeWidth={1.75} aria-hidden="true" />
              <span>New chat</span>
            </Sidebar.MenuButton>
          </Sidebar.MenuItem>
          <Sidebar.MenuItem>
            <Sidebar.MenuButton class="pointer-events-none opacity-45" tooltipContent="History placeholder">
              <MessageSquareIcon size={16} strokeWidth={1.75} aria-hidden="true" />
              <span>Recent thread</span>
            </Sidebar.MenuButton>
          </Sidebar.MenuItem>
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