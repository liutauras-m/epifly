import type { ConusSdk } from "@conusai/sdk";
import { createThreadsStore } from "../threads/threads.store.svelte.js";
import { sortByRecent } from "../threads/threads.utils.js";
import { createWorkspacesStore } from "../workspaces/workspaces.store.svelte.js";
import { toSidebarWorkspaceNode } from "../workspaces/workspace-adapters.js";
import type { SidebarWorkspaceNode } from "../workspaces/workspace-adapters.js";

type Navigate = (href: string) => void | Promise<void>;

export type CreateAppShellStateArgs = {
  sdk: ConusSdk;
  /** Reactive getter for the current pathname — pass `() => page.url.pathname`. */
  getPathname: () => string;
  /** Reactive getter for the active thread id — pass `() => page.params.threadId ?? null`. */
  getThreadId: () => string | null;
  /**
   * Navigation callback injected by the consuming app. Keeps this module free
   * of `$app/navigation` so it works in both `apps/web` and `apps/native`
   * without pulling SvelteKit internals into the shared features package.
   */
  navigate: Navigate;
};

export type { SidebarWorkspaceNode };

/**
 * Reactive shell state for app layouts.
 *
 * Owns sidebar data derivations and navigation actions so that
 * `+layout.svelte` stays thin — compose shell, nothing else.
 *
 * ```svelte
 * const shell = createAppShellState({ sdk, getPathname, getThreadId, navigate: goto });
 * onMount(shell.load);
 * ```
 */
export function createAppShellState(args: CreateAppShellStateArgs) {
  const threadsStore = createThreadsStore(args.sdk);
  const workspacesStore = createWorkspacesStore(args.sdk);

  // Route-derived — reactive because the getters are called inside $derived.
  const activePath = $derived(args.getPathname());
  const activeThreadId = $derived(args.getThreadId());

  // Sidebar data — pure transformations, never side effects.
  const sortedThreads = $derived(sortByRecent(threadsStore.threads));
  const workspaceNodes = $derived<SidebarWorkspaceNode[]>(
    workspacesStore.tree.map(toSidebarWorkspaceNode)
  );

  /** Call once from the layout's onMount. Stores dedupe internally. */
  function load() {
    threadsStore.loadOnce({ limit: 20 });
    workspacesStore.loadTreeOnce(null);
    threadsStore.connectRealtime();
    workspacesStore.connectRealtime();

    return () => {
      threadsStore.disconnectRealtime();
      workspacesStore.disconnectRealtime();
    };
  }

  // Navigation — delegates to injected navigate, no direct SvelteKit dep.
  function goToNewChat() { args.navigate("/"); }
  function goToThread(id: string) { args.navigate(`/chat/${id}`); }
  function selectWorkspaceNode(id: string) { void workspacesStore.selectAndLoadNode(id); }
  function createWorkspaceNode(kind: "folder" | "document", name: string, parentId?: string | null) {
    return workspacesStore.createNode(kind, name, parentId);
  }

  return {
    get activePath() { return activePath; },
    get activeThreadId() { return activeThreadId; },
    get sortedThreads() { return sortedThreads; },
    get workspaceNodes() { return workspaceNodes; },
    get threadsLoading() { return threadsStore.isLoading; },
    get workspaceLoading() { return workspacesStore.isLoading; },
    get workspaceCreating() { return workspacesStore.isCreating; },
    get workspaceError() { return workspacesStore.error; },
    get selectedWorkspaceNodeId() { return workspacesStore.selectedNodeId; },
    load,
    goToNewChat,
    goToThread,
    selectWorkspaceNode,
    createWorkspaceNode
  };
}
