<script lang="ts">
	import '@conusai/ui/foundry.css';
	import { ThemeProvider, LiveAnnouncer, ToastHost } from '@conusai/ui';
	import { emit } from '@tauri-apps/api/event';

	async function handleThemeChange(theme: string) {
		try { await emit('theme-change', { theme }); } catch { /* not in Tauri */ }
	}

	let { children } = $props();

	// Signal hydration complete — mirrors the web app so shared E2E selectors work.
	$effect(() => {
		document.documentElement.dataset.hydrated = 'true';
	});
</script>

<ThemeProvider onThemeChange={handleThemeChange}>
	<div class="shell-root">
		{@render children()}
	</div>
	<LiveAnnouncer />
	<ToastHost />
</ThemeProvider>

<style>
	.shell-root {
		display: flex;
		flex-direction: column;
		height: 100dvh;
		overflow: hidden;
	}
</style>
