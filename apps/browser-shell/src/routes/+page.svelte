<script lang="ts">
	import { onMount } from 'svelte';
	import { modeStore, screenStore, breadcrumbsStore, recentsStore, toasts } from '@conusai/ui/stores';
	import {
		createChatStream,
		ShellScreen,
		ShellLoginScreen,
		initialRoute,
		applyInitialRoute,
	} from '@conusai/ui/features';
	import type { WorkspaceNode } from '@conusai/types';
	import { sdk, getSessionToken } from '$lib/sdk.js';
	import { isTauri, streamChatTauri } from '$lib/tauri-stream.js';
	import { user, initAuth, login, logout } from '$lib/auth.svelte.js';
	import { setPlatformTag } from '$lib/mobile/platform/detect.js';
	import logoDark from '@conusai/ui/assets/images/conusai-logo-darkmode.png';
	import favicon from '@conusai/ui/assets/images/favicon.png';

	modeStore.setMode('shell');

	// ── Tauri streaming ──────────────────────────────────────────────────────
	const tauriStreamFn = isTauri
		? (params: import('@conusai/sdk').StreamChatParams) =>
				streamChatTauri({
					message: params.message,
					sessionToken: getSessionToken() ?? '',
					threadId: params.threadId,
					workspaceNodeId: params.workspaceNodeId,
					attachmentIds: params.attachmentIds,
					forcedCapability: params.forcedCapability,
					signal: params.signal,
				})
		: undefined;

	const chatStream = createChatStream(sdk, { streamFn: tauriStreamFn });

	// ── Bindable selected node (seeded from deep-link restore) ───────────────
	let selectedNode = $state<WorkspaceNode | null>(null);

	function handleLogout() {
		chatStream.newSession();
		logout();
	}

	// ── Mount: platform tag + auth restore + deep-link route restore ─────────
	onMount(async () => {
		setPlatformTag();
		await initAuth();

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
				if (typeof window !== 'undefined' && window.location.search.includes('ws=')) {
					window.history.replaceState({}, '', window.location.pathname);
				}
			},
		});
		if (route.cap) screenStore.setActive('chat');
	});
</script>

{#if !user}
	<ShellLoginScreen
		onSubmit={async (name, plan) => { await login(name, plan); }}
		logoSrc={logoDark}
	/>
{:else}
	<ShellScreen
		{sdk}
		{chatStream}
		userName={user.name}
		userPlan={user.plan}
		sigil={favicon}
		onLogout={handleLogout}
		bind:selectedNode
	/>
{/if}
