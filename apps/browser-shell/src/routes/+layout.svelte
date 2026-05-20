<script lang="ts">
	import '@conusai/ui/tokens.css';
	import '@conusai/ui/foundry.css';
	import { ThemeProvider, LiveAnnouncer } from '@conusai/ui';
	import { emit } from '@tauri-apps/api/event';

	async function handleThemeChange(theme: string) {
		try { await emit('theme-change', { theme }); } catch { /* not in Tauri */ }
	}

	let { children } = $props();
</script>

<ThemeProvider onThemeChange={handleThemeChange}>
	<div class="shell-root">
		{@render children()}
	</div>
	<LiveAnnouncer />
</ThemeProvider>

<style>
	.shell-root {
		display: flex;
		flex-direction: column;
		height: 100dvh;
		overflow: hidden;
	}
</style>
