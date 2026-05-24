<script lang="ts">
	import '../app.css';
	import '@conusai/ui/foundry.css';
	import { ThemeProvider, LiveAnnouncer, ToastHost, setI18n, createI18n, enMessages } from '@conusai/ui';
	import type { LayoutData } from './$types';
	let { data, children }: { data: LayoutData; children: import('svelte').Snippet } = $props();

	// Bootstrap i18n with built-in English messages (Phase 7).
	// Runs at module evaluation time — before any component renders — so that
	// t('composer.placeholder') etc. resolve on both SSR and CSR.
	setI18n(createI18n('en', enMessages));

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
