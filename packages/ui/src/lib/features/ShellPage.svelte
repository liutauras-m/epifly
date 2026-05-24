<svelte:options runes={true} />
<script lang="ts">
  /**
   * ShellPage — single mount-point for the authenticated workshop route.
   *
   * Absorbs the boilerplate that previously lived (in slightly different
   * forms) in both apps' root +page.svelte:
   *   - selectedNode state
   *   - deep-link restoration via initialRoute + applyInitialRoute
   *   - workspace ↔ URL sync via callback (router-agnostic)
   *
   * Each app supplies its own router via `onWorkspaceChange` / `onUnknownRoute`
   * (SvelteKit `goto` for web, `history.replaceState` for the Tauri shell).
   *
   * Everything else — sdk, chatStream, auth, sigil — comes from the consumer.
   */
  import { onMount } from 'svelte';
  import type { ConusSdk } from '@conusai/sdk';
  import type { WorkspaceNode } from '@conusai/types';

  import ShellScreen from './ShellScreen.svelte';
  import { initialRoute }       from '../routing/initialRoute.js';
  import { applyInitialRoute }  from '../routing/applyInitialRoute.js';
  import {
    screenStore,
    breadcrumbsStore,
    recentsStore,
    toasts,
  } from '../stores/index.js';

  let {
    sdk,
    chatStream,
    userName,
    userPlan = '',
    sigil,
    appTitle = 'ConusAI',
    onLogout,
    onWorkspaceChange,
    onUnknownRoute,
  }: {
    sdk:          ConusSdk;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    chatStream:   any;
    userName:     string;
    userPlan?:    string;
    sigil?:       string;
    appTitle?:    string;
    onLogout?:    () => void;
    /** Called when the selected workspace changes — apps use this to sync URL. */
    onWorkspaceChange?: (workspaceId: string | null) => void;
    /** Called when applyInitialRoute can't resolve `?ws=<id>` — apps clear the URL. */
    onUnknownRoute?:    () => void;
  } = $props();

  let selectedNode = $state<WorkspaceNode | null>(null);

  // Workspace → URL sync (router-agnostic via callback).
  $effect(() => {
    onWorkspaceChange?.(selectedNode?.id ?? null);
  });

  // Deep-link restore — runs once after mount.
  onMount(async () => {
    const route = await initialRoute();
    await applyInitialRoute<WorkspaceNode>(sdk, route, {
      onApplyNode(node) {
        selectedNode = node;
        breadcrumbsStore.set(node);
        recentsStore.add(node.id);
        screenStore.setActive('chat');
        if (node.kind === 'conversation' && (node as any).metadata?.thread_id) {
          chatStream.loadThread?.((node as any).metadata.thread_id);
        }
      },
      onUnknown() {
        toasts.warning('Workspace not found, returning to root');
        onUnknownRoute?.();
      },
    });
    if (route.cap) screenStore.setActive('chat');
  });
</script>

<ShellScreen
  {sdk}
  {chatStream}
  {userName}
  {userPlan}
  {sigil}
  {appTitle}
  {onLogout}
  bind:selectedNode
/>
