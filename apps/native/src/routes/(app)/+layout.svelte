<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { getSdkContext, createAppShellState, setWorkspaceNodeContext, setActiveThreadNodeContext, setWorkspaceActionsContext } from "@epifly/features";
  import { AppJobsSidebar, AppMain, AppNavigationSidebar, AppShell, WorkspaceCommandPalette, WorkspaceDocPeek } from "@epifly/ui";
  import type { PaletteCommand } from "@epifly/ui";
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
  // Expose workspace write actions so chat pages can insert optimistic nodes (Step 7.1).
  setWorkspaceActionsContext({ insertOptimisticThread: shell.insertOptimisticThread });

  onMount(shell.load);

  // Command palette (Step 3.5)
  let paletteOpen = $state(false);

  const paletteCommands: PaletteCommand[] = [
    { id: "new-chat",       label: "New chat",             shortcut: "⌘N", group: "Navigation", onRun: () => shell.goToNewChat() },
    { id: "search",         label: "Search workspace",     shortcut: "⌘F", group: "Navigation", onRun: () => paletteOpen = false },
    { id: "new-folder",     label: "New folder",                            group: "Organize",   onRun: () => paletteOpen = false },
    // Phase 8.3 — Smart Views
    { id: "view-unsorted",  label: "Unsorted conversations",                group: "Views",      onRun: () => { shell.selectSmartView("unsorted"); paletteOpen = false; } },
    { id: "view-review",    label: "Needs review",                          group: "Views",      onRun: () => { shell.selectSmartView("needs-review"); paletteOpen = false; } },
    { id: "view-recent",    label: "Recently updated",                      group: "Views",      onRun: () => { shell.selectSmartView("recently-updated"); paletteOpen = false; } },
  ];

  function handleGlobalKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === "k") {
      event.preventDefault();
      paletteOpen = !paletteOpen;
    }
  }
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
    onMoveWorkspaceNode={(src, tgt) => { void shell.moveWorkspaceNode(src, tgt, null); }}
    onRenameWorkspaceNode={(id, name) => { void shell.renameWorkspaceNode(id, name); }}
    onDeleteWorkspaceNode={(id, isThread) => { void shell.deleteWorkspaceNode(id, isThread); }}
    onRestoreThread={(threadId) => { void shell.restoreThread(threadId); }}
    onViewDocRequest={(nodeId, name, summary) => shell.openPeek(nodeId, name, summary)}
    onSetWorkspaceNodeStatus={(id, status) => { void shell.setNodeStatus(id, status); }}
    onWorkspaceNodeSelect={shell.selectWorkspaceNode}
    onWorkspaceNodeCreate={shell.createWorkspaceNode}
    onSearch={shell.searchWorkspace}
    smartViewActive={shell.smartViewActive}
    smartViewResults={shell.smartViewResults}
    smartViewLoading={shell.smartViewLoading}
    smartViewError={shell.smartViewError}
    onSelectSmartView={shell.selectSmartView}
    onClearSmartView={shell.clearSmartView}
  />
{/snippet}

{#snippet rightSidebar()}
  <AppJobsSidebar />
{/snippet}

<svelte:window onkeydown={handleGlobalKeydown} />

<AppShell {sidebar} {rightSidebar}>
  <AppMain class="bg-background">
    {@render children?.()}
  </AppMain>
</AppShell>

<!-- Phase 4.1 — "View as document" peek panel (rendered at layout level for correct z-stacking) -->
<WorkspaceDocPeek
  open={shell.peekOpen}
  nodeName={shell.peekNodeName}
  summary={shell.peekSummary}
  content={shell.peekContent}
  isLoading={shell.peekLoading}
  error={shell.peekError}
  onClose={shell.closePeek}
/>

<WorkspaceCommandPalette
  open={paletteOpen}
  commands={paletteCommands}
  onClose={() => (paletteOpen = false)}
/>
