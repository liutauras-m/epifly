<script lang="ts">
	import { onMount } from 'svelte';
	import { replaceState } from '$app/navigation';
	import { page } from '$app/state';
	import { ShellPage, ShellLoginScreen, createChatStream, type CustomStreamFn } from '@conusai/ui/features';
	import { sdk, getSessionToken } from '$lib/sdk.js';
	import { isTauri, streamChatTauri } from '$lib/tauri-stream.js';
	import { auth, initAuth, login, logout } from '$lib/auth.svelte.js';
	import { setPlatformTag } from '$lib/mobile/platform/detect.js';
	import logoDark from '@conusai/ui/assets/images/conusai-logo-darkmode.png';
	import favicon from '@conusai/ui/assets/images/favicon.png';

	// ── Tauri streaming (only when running inside the Tauri webview) ──────────
	const tauriStreamFn: CustomStreamFn | undefined = isTauri
		? (params) =>
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

	function handleLogout() {
		chatStream.newSession();
		logout();
	}

	function syncWorkspaceToUrl(wsId: string | null) {
		if (typeof window === 'undefined') return;
		const url = new URL(page.url);
		if (wsId) {
			if (url.searchParams.get('ws') === wsId) return;
			url.searchParams.set('ws', wsId);
		} else {
			if (!url.searchParams.has('ws')) return;
			url.searchParams.delete('ws');
		}
		replaceState(url, page.state);
	}

	onMount(async () => {
		setPlatformTag();
		await initAuth();
	});
</script>

{#if !auth.user}
	<ShellLoginScreen
		onSubmit={async (name, plan) => { await login(name, plan); }}
		logoSrc={logoDark}
	/>
{:else}
	<ShellPage
		{sdk}
		{chatStream}
		userName={auth.user.name}
		userPlan={auth.user.plan}
		sigil={favicon}
		onLogout={handleLogout}
		onWorkspaceChange={syncWorkspaceToUrl}
		onUnknownRoute={() => syncWorkspaceToUrl(null)}
	/>
{/if}
