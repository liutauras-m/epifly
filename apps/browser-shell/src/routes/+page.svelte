<script lang="ts">
	import { onMount } from 'svelte';
	import { replaceState } from '$app/navigation';
	import { page } from '$app/state';
	import { ShellPage, ShellLoginScreen, createChatStream, type CustomStreamFn } from '@conusai/ui/features';
	import { useAppStores } from '@conusai/ui/stores';
	import { sdk } from '$lib/sdk.js';
	import { isTauri, streamChatTauri } from '$lib/tauri-stream.js';
	import { session, restoreSession, signIn, signOut } from '$lib/session.svelte.js';
	import { setPlatformTag } from '$lib/mobile/platform/detect.js';
	import logoDark from '@conusai/ui/assets/images/conusai-logo-darkmode.png';
	import favicon from '@conusai/ui/assets/images/favicon.png';

	const stores = useAppStores();

	// ── Tauri streaming (only when running inside the Tauri webview) ──────────
	// Rust adds the gateway URL and the bearer token; no token passes through here.
	const tauriStreamFn: CustomStreamFn | undefined = isTauri
		? (params) =>
				streamChatTauri({
					message: params.message,
					threadId: params.threadId,
					workspaceNodeId: params.workspaceNodeId,
					attachmentIds: params.attachmentIds,
					forcedCapability: params.forcedCapability,
					signal: params.signal,
				})
		: undefined;

	const chatStream = createChatStream(sdk, { toasts: stores.toasts, streamFn: tauriStreamFn });

	async function handleLogout() {
		chatStream.newSession();
		// Reset navigation so the next sign-in starts clean.
		stores.breadcrumbs.clear();
		stores.recents.clear();
		stores.drawer.close();
		await signOut();
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
		await restoreSession();
	});
</script>

{#if session.status === 'loading'}
	<!-- Checking the vault — renders nothing for a frame. -->
{:else if !session.user}
	<ShellLoginScreen
		onSignIn={signIn}
		busy={session.status === 'signing-in'}
		error={session.error}
		logoSrc={logoDark}
	/>
{:else}
	<ShellPage
		{sdk}
		{chatStream}
		userName={session.user.name}
		sigil={favicon}
		onLogout={handleLogout}
		onWorkspaceChange={syncWorkspaceToUrl}
		onUnknownRoute={() => syncWorkspaceToUrl(null)}
	/>
{/if}
