<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { getSdkContext, createAppShellState, setWorkspaceNodeContext, setActiveThreadNodeContext } from "@epifly/features";
  import { AppJobsSidebar, AppMain, AppNavigationSidebar, AppShell } from "@epifly/ui";
  import type { Snippet } from "svelte";

  type Props = { children?: Snippet };
  let { children }: Props = $props();

  const sdk = getSdkContext();

  const shell = createAppShellState({
    sdk,
    getPathname: () => page.url.pathname,
    getThreadId: () => page.params.threadId ?? null,
    navigate: goto
  });

  // Expose the selected workspace node to child pages via context.
  setWorkspaceNodeContext(() => shell.selectedWorkspaceNodeId);
  // Expose the active thread's workspace location (breadcrumb + context indicator).
  setActiveThreadNodeContext(() => shell.activeThreadNode);

  onMount(shell.load);
</script>

{#snippet sidebar()}
  <AppNavigationSidebar
    activePath={shell.activePath}
    threads={shell.sortedThreads}
    threadsLoading={shell.threadsLoading}
    workspaceNodes={shell.workspaceNodes}
    workspaceLoading={shell.workspaceLoading}
    workspaceCreating={shell.workspaceCreating}
    workspaceError={shell.workspaceError}
    activeThreadId={shell.activeThreadId}
    selectedWorkspaceNodeId={shell.selectedWorkspaceNodeId}
    onNewChat={shell.goToNewChat}
    onThreadSelect={shell.goToThread}
    onOpenThread={shell.goToThread}
    onWorkspaceNodeSelect={shell.selectWorkspaceNode}
    onWorkspaceNodeCreate={shell.createWorkspaceNode}
    onSearch={shell.searchWorkspace}
  />
{/snippet}

{#snippet rightSidebar()}
  <AppJobsSidebar />
{/snippet}

<AppShell {sidebar} {rightSidebar}>
  <AppMain class="bg-background">
    {@render children?.()}
  </AppMain>
</AppShell>
