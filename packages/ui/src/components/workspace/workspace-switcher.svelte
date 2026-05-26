<script lang="ts">
  import CheckIcon from "@lucide/svelte/icons/check";
  import ChevronsUpDownIcon from "@lucide/svelte/icons/chevrons-up-down";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import * as DropdownMenu from "../ui/dropdown-menu/index.js";
  import * as Button from "../ui/button/index.js";

  export type Workspace = {
    id: string;
    name: string;
    logo?: string;
  };

  type Props = {
    workspaces: Workspace[];
    activeId?: string;
    onselect?: (id: string) => void;
    oncreate?: () => void;
  };

  let { workspaces, activeId, onselect, oncreate }: Props = $props();

  let active = $derived(workspaces.find((w) => w.id === activeId) ?? workspaces[0]);
</script>

<DropdownMenu.DropdownMenu>
  <DropdownMenu.DropdownMenuTrigger>
    {#snippet child({ props })}
      <Button.Button
        {...props}
        variant="ghost"
        class="flex h-auto w-full items-center gap-2 px-2 py-1.5 text-sm font-medium"
      >
        <!-- Workspace icon / logo -->
        <span
          class="flex h-6 w-6 shrink-0 items-center justify-center rounded bg-primary text-[10px] font-bold uppercase text-primary-foreground"
          aria-hidden="true"
        >
          {active?.name?.slice(0, 2) ?? "WS"}
        </span>
        <span class="flex-1 truncate text-left">{active?.name ?? "Select workspace"}</span>
        <ChevronsUpDownIcon class="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.75} aria-hidden="true" />
      </Button.Button>
    {/snippet}
  </DropdownMenu.DropdownMenuTrigger>

  <DropdownMenu.DropdownMenuContent class="w-56" align="start">
    <DropdownMenu.DropdownMenuLabel>Workspaces</DropdownMenu.DropdownMenuLabel>
    <DropdownMenu.DropdownMenuSeparator />
    {#each workspaces as ws (ws.id)}
      <DropdownMenu.DropdownMenuItem
        onclick={() => onselect?.(ws.id)}
      >
        <span
          class="mr-2 flex h-5 w-5 shrink-0 items-center justify-center rounded bg-primary text-[9px] font-bold uppercase text-primary-foreground"
          aria-hidden="true"
        >
          {ws.name.slice(0, 2)}
        </span>
        <span class="flex-1 truncate">{ws.name}</span>
        {#if ws.id === activeId}
          <CheckIcon class="ml-auto size-3.5" strokeWidth={1.75} aria-label="Active" />
        {/if}
      </DropdownMenu.DropdownMenuItem>
    {/each}
    {#if oncreate}
      <DropdownMenu.DropdownMenuSeparator />
      <DropdownMenu.DropdownMenuItem onclick={oncreate}>
        <PlusIcon class="mr-2 size-4" strokeWidth={1.75} aria-hidden="true" />
        New workspace
      </DropdownMenu.DropdownMenuItem>
    {/if}
  </DropdownMenu.DropdownMenuContent>
</DropdownMenu.DropdownMenu>
