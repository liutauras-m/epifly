<script lang="ts">
  import { onMount } from "svelte";
  import { getSdkContext, createWorkspacesStore, toSidebarWorkspaceNode } from "@epifly/features";
  import { AppSafeArea, WorkspaceTree, Skeleton } from "@epifly/ui";

  const sdk = getSdkContext();
  const workspaces = createWorkspacesStore(sdk);
  const workspaceNodes = $derived(workspaces.tree.map(toSidebarWorkspaceNode));

  onMount(() => workspaces.loadTreeOnce(null));
</script>

<svelte:head>
  <title>Workspace · Epifly</title>
</svelte:head>

<AppSafeArea class="flex min-h-0 flex-1 flex-col overflow-y-auto">
  <div class="px-6 pb-8 pt-[calc(var(--sidebar-toggle-offset)+2.75rem)]">
    <div class="mb-6 flex items-baseline justify-between">
      <h1 class="text-2xl font-semibold tracking-tight">Workspace</h1>
      {#if workspaces.error}
        <p class="text-xs text-destructive">{workspaces.error}</p>
      {/if}
    </div>

    {#if workspaces.isLoading}
      <div class="space-y-2" aria-label="Loading workspace files">
        {#each [1, 2, 3, 4, 5] as i (i)}
          <Skeleton.Skeleton class="h-8 w-full rounded-lg" />
        {/each}
      </div>
    {:else if workspaces.tree.length === 0}
      <div class="flex flex-col items-center justify-center py-16 text-center">
        <p class="text-sm font-medium text-foreground">No files yet</p>
        <p class="mt-1 text-xs text-muted-foreground">
          Files and folders you add will appear here.
        </p>
      </div>
    {:else}
      <WorkspaceTree nodes={workspaceNodes} />
    {/if}
  </div>
</AppSafeArea>
