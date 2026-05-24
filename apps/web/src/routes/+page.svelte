<svelte:options runes={true} />
<script lang="ts">
	import { goto } from '$app/navigation';
	import type { PageData } from './$types.js';
	import { sdk } from '$lib/sdk.js';
	import { provideCapabilityRendererRegistry } from '@conusai/ui/capabilities';
	import { ShellPage, createChatStream } from '@conusai/ui/features';
	import favicon from '@conusai/ui/assets/images/favicon.png';

	let { data }: { data: PageData } = $props();

	provideCapabilityRendererRegistry();
	const chatStream = createChatStream(sdk, { tenantId: data.user?.tenantId ?? null });

	function syncWorkspaceToUrl(wsId: string | null) {
		if (typeof window === 'undefined') return;
		const url = new URL(window.location.href);
		if (wsId) {
			if (url.searchParams.get('ws') !== wsId) {
				goto(`?ws=${wsId}`, { replaceState: true, keepFocus: true, noScroll: true });
			}
		} else if (url.searchParams.has('ws')) {
			goto('/', { replaceState: true, keepFocus: true, noScroll: true });
		}
	}
</script>

<svelte:head><title>Workshop · ConusAI</title></svelte:head>

<ShellPage
	{sdk}
	{chatStream}
	userName={data.user?.name ?? 'Operator'}
	userPlan={data.user?.plan ?? ''}
	sigil={favicon}
	appTitle="Workshop"
	onLogout={() => goto('/logout')}
	onWorkspaceChange={syncWorkspaceToUrl}
	onUnknownRoute={() => goto('/', { replaceState: true, keepFocus: true, noScroll: true })}
/>
