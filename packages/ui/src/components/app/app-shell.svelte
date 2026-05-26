<script lang="ts">
  import PanelRightIcon from "@lucide/svelte/icons/panel-right";
  import * as Button from "../ui/button/index.js";
  import * as Sheet from "../ui/sheet/index.js";
  import * as Sidebar from "../ui/sidebar/index.js";
  import type { Snippet } from "svelte";

  type Props = {
    children?: Snippet;
    sidebar?: Snippet;
    rightSidebar?: Snippet;
  };

  let { children, sidebar, rightSidebar }: Props = $props();
  let rightOpen = $state(false);
  let rightMobileOpen = $state(false);
</script>

<Sidebar.Provider>
  {#if sidebar}
    {@render sidebar()}
    <Sidebar.Trigger
      class="app-sidebar-toggle app-sidebar-toggle-left hover:bg-accent"
    />
  {/if}
  <Sidebar.Inset class="h-svh min-h-0 overflow-hidden">
    {@render children?.()}
  </Sidebar.Inset>
  {#if rightSidebar}
    <aside
      class="app-jobs-sidebar hidden h-svh shrink-0 border-l border-border bg-background md:flex"
      aria-label="Jobs sidebar"
      data-open={rightOpen}
      aria-hidden={!rightOpen}
    >
      <div class="w-[var(--jobs-sidebar-width)] shrink-0">
        {@render rightSidebar()}
      </div>
    </aside>

    <Sheet.Root bind:open={rightMobileOpen}>
      <Sheet.Content side="right" class="w-[min(20rem,calc(100vw-2rem))] p-0 md:hidden">
        <Sheet.Header class="sr-only">
          <Sheet.Title>Jobs</Sheet.Title>
          <Sheet.Description>Job activity and queued work.</Sheet.Description>
        </Sheet.Header>
        {@render rightSidebar()}
      </Sheet.Content>
    </Sheet.Root>

    <Button.Button
      type="button"
      variant="ghost"
      size="icon-sm"
      aria-label="Toggle Jobs Sidebar"
      aria-pressed={rightOpen}
      onclick={() => (rightOpen = !rightOpen)}
      data-open={rightOpen}
      class="app-sidebar-toggle app-sidebar-toggle-right hidden hover:bg-accent md:inline-flex"
    >
      <PanelRightIcon size={16} strokeWidth={1.75} aria-hidden="true" />
    </Button.Button>

    <Button.Button
      type="button"
      variant="ghost"
      size="icon-sm"
      aria-label="Toggle Jobs Sidebar"
      onclick={() => (rightMobileOpen = true)}
      class="app-sidebar-toggle app-sidebar-toggle-right hover:bg-accent md:hidden"
    >
      <PanelRightIcon size={16} strokeWidth={1.75} aria-hidden="true" />
    </Button.Button>
  {/if}
</Sidebar.Provider>
