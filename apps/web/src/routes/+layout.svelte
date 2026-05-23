<script lang="ts">
	import '@conusai/ui/foundry.css';
	import { ThemeProvider, LiveAnnouncer, ToastHost } from '@conusai/ui';
	import type { LayoutData } from './$types';
	let { data, children }: { data: LayoutData; children: import('svelte').Snippet } = $props();

	// Set a data attribute once Svelte hydration is complete so E2E tests can wait on it.
	// This runs after all child components are mounted (effects run innermost → outermost).
	$effect(() => {
		document.documentElement.dataset.hydrated = 'true';
	});
</script>
<ThemeProvider>
	{@render children()}
</ThemeProvider>
<LiveAnnouncer />
<ToastHost />
