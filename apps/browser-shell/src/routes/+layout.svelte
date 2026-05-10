<script lang="ts">
	import '@conusai/ui/foundry.css';
	import { AppShell, TabStrip, RecorderControls, ThemeProvider, LiveAnnouncer } from '@conusai/ui';
	import { toasts } from '@conusai/ui/stores';
	import { invoke } from '@tauri-apps/api/core';
	import { emit, listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import type { SessionTrace } from '@conusai/types';

	// Detect mobile at render time — safe because adapter-static means no SSR.
	let isMobile = $state(typeof window !== 'undefined' ? window.innerWidth <= 640 : false);

	let tabs = $state<{ id: string; label: string; url: string }[]>([]);
	let activeTabId = $state<string | undefined>(undefined);
	let recorderState = $state<'idle' | 'recording' | 'uploading'>('idle');
	let stepCount = $state(0);

	let screenshotInterval: ReturnType<typeof setInterval> | undefined;

	async function handleThemeChange(theme: string) {
		await emit('theme-change', { theme });
	}

	onMount(() => {
		// Update on resize (e.g. Simulator window resize in dev).
		const mq = window.matchMedia('(max-width: 640px)');
		const onMq = (e: MediaQueryListEvent) => { isMobile = e.matches; };
		mq.addEventListener('change', onMq);

		// Desktop only: listen for shell-ready to restore tabs.
		const unlistenPromise = !isMobile ? listen('shell-ready', async () => {
			await restorePersistedTabs();
		}) : Promise.resolve(() => {});

		return () => {
			mq.removeEventListener('change', onMq);
			unlistenPromise.then(fn => fn());
		};
	});

	async function restorePersistedTabs() {
		try {
			const saved = await invoke<{ id: string; url: string; title: string }[]>('restore_tabs');
			for (const tab of saved) {
				const id = await invoke<string>('create_tab', { url: tab.url });
				tabs = [...tabs, { id, label: tab.title, url: tab.url }];
				if (!activeTabId) activeTabId = id;
			}
		} catch { /* No persisted tabs. */ }
	}

	async function handleNewTab() {
		try {
			const id = await invoke<string>('create_tab', { url: 'https://example.com' });
			tabs = [...tabs, { id, label: 'New Tab', url: 'https://example.com' }];
			activeTabId = id;
			await invoke('save_tabs');
		} catch (e) { toasts.error(`Failed to open tab: ${e}`); }
	}

	async function handleCloseTab(id: string) {
		await invoke('close_tab', { id });
		tabs = tabs.filter(t => t.id !== id);
		if (activeTabId === id) activeTabId = tabs[0]?.id;
		await invoke('save_tabs');
	}

	function startScreenshotPolling() {
		if (!activeTabId) return;
		const tabId = activeTabId;
		screenshotInterval = setInterval(async () => {
			if (recorderState !== 'recording') { stopScreenshotPolling(); return; }
			try { await invoke('capture_tab_screenshot', { tabId }); } catch {}
		}, 1000);
	}

	function stopScreenshotPolling() {
		if (screenshotInterval !== undefined) { clearInterval(screenshotInterval); screenshotInterval = undefined; }
	}

	async function handleStartRecording() {
		await invoke('recorder_start');
		recorderState = 'recording';
		stepCount = 0;
		startScreenshotPolling();
		const interval = setInterval(async () => {
			if (recorderState !== 'recording') { clearInterval(interval); return; }
			const status = await invoke<{ recording: boolean; step_count: number }>('recorder_status');
			stepCount = status.step_count;
		}, 500);
	}

	async function handleStopRecording() {
		stopScreenshotPolling();
		recorderState = 'uploading';
		try {
			const trace = await invoke<SessionTrace | null>('recorder_stop');
			if (trace) {
				const nodeId = await invoke<string>('upload_trace_cmd', { trace });
				toasts.success(`Trace saved — workspace node ${nodeId.slice(0, 8)}…`);
			}
		} catch (e) { toasts.error(`Upload failed: ${e}`); }
		finally { recorderState = 'idle'; stepCount = 0; }
	}

	let { children } = $props();
</script>

<ThemeProvider onThemeChange={handleThemeChange}>
	{#if isMobile}
		<!-- Mobile: no AppShell chrome, children own the full screen -->
		<div class="mobile-root">
			{@render children()}
		</div>
	{:else}
		<!-- Desktop: full AppShell with tabs + recorder -->
		<AppShell title="ConusAI Browser">
			{#snippet sidebar()}
				<div class="shell-sidebar">
					<div class="sidebar-header">
						<span class="logo">ConusAI</span>
					</div>
					<RecorderControls
						state={recorderState}
						{stepCount}
						onstart={handleStartRecording}
						onstop={handleStopRecording}
					/>
				</div>
			{/snippet}

			<div class="shell-content">
				<TabStrip
					{tabs}
					activeId={activeTabId}
					onselect={(id) => (activeTabId = id)}
					onclose={handleCloseTab}
					oncreate={handleNewTab}
				/>
				<div class="page">
					{@render children()}
				</div>
			</div>
		</AppShell>
	{/if}

	<LiveAnnouncer />
</ThemeProvider>

<style>
	.mobile-root {
		display: flex;
		flex-direction: column;
		height: 100dvh;
		overflow: hidden;
	}

	.shell-sidebar {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.sidebar-header {
		display: flex;
		align-items: center;
		gap: var(--s-2);
		padding: var(--s-4);
		border-bottom: 1px solid var(--rule);
	}

	.logo {
		font-family: var(--font-display);
		font-size: 18px;
		color: var(--ember);
		flex: 1;
	}

	.shell-content {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.page {
		flex: 1;
		overflow: auto;
	}
</style>
