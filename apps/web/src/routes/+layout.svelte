<script lang="ts">
	import '@conusai/ui/foundry.css';
	import { ThemeProvider, LiveAnnouncer } from '@conusai/ui';
	import favicon from '@conusai/ui/assets/images/favicon.png';
	import type { LayoutData } from './$types';
	let { data, children }: { data: LayoutData; children: import('svelte').Snippet } = $props();

	// Set a data attribute once Svelte hydration is complete so E2E tests can wait on it.
	// This runs after all child components are mounted (effects run innermost → outermost).
	$effect(() => {
		document.documentElement.dataset.hydrated = 'true';
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} type="image/png">
</svelte:head>

<ThemeProvider>
	{@render children()}
</ThemeProvider>
<LiveAnnouncer />
