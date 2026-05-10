<script lang="ts">
	import '@conusai/ui/foundry.css';
	import { AppShell, TabStrip, RecorderControls, ToastHost, ThemeProvider, LiveAnnouncer } from '@conusai/ui';
	import { toasts } from '@conusai/ui/stores';
	import { invoke } from '@tauri-apps/api/core';
	import { emit, listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import type { SessionTrace } from '@conusai/types';

	async function handleThemeChange(theme: string) {
		await emit('theme-change', { theme });
	}

	let tabs = $state<{ id: string; label: string; url: string }[]>([]);
	let activeTabId = $state<string | undefined>(undefined);
	let recorderState = $state<'idle' | 'recording' | 'uploading'>('idle');
	let stepCount = $state(0);
	let shellReady = $state(false);

	let screenshotInterval: ReturnType<typeof setInterval> | undefined;

	onMount(() => {
		const unlistenPromise = listen('shell-ready', async () => {
			shellReady = true;
			await loadTokenFromStronghold();
			await restorePersistedTabs();
		});
		return () => { unlistenPromise.then(fn => fn()); };
	});

	async function loadTokenFromStronghold() {
		try {
			const { Client } = await import('@tauri-apps/plugin-stronghold');
			const { appDataDir } = await import('@tauri-apps/api/path');
			const vaultPath = (await appDataDir()) + '/conusai.stronghold';
			const client = await Client.load(vaultPath, 'conusai-shell-v1');
			const store = client.getStore('tokens');
			const raw = await store.get('device_token');
			if (raw) {
				const token = new TextDecoder().decode(new Uint8Array(raw));
				await invoke('set_device_token', { token });
			}
		} catch {
			// Vault not yet provisioned — page will show LoginPanel.
		}
	}

	async function restorePersistedTabs() {
		try {
			const saved = await invoke<{ id: string; url: string; title: string }[]>('restore_tabs');
			for (const tab of saved) {
				const id = await invoke<string>('create_tab', { url: tab.url });
				tabs = [...tabs, { id, label: tab.title, url: tab.url }];
				if (!activeTabId) activeTabId = id;
			}
		} catch {
			// No persisted tabs.
		}
	}

	async function handleNewTab() {
		try {
			const id = await invoke<string>('create_tab', { url: 'https://example.com' });
			tabs = [...tabs, { id, label: 'New Tab', url: 'https://example.com' }];
			activeTabId = id;
			await invoke('save_tabs');
		} catch (e) {
			toasts.error(`Failed to open tab: ${e}`);
		}
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
		} catch (e) {
			toasts.error(`Upload failed: ${e}`);
		} finally {
			recorderState = 'idle';
			stepCount = 0;
		}
	}

	let { children } = $props();
</script>

<ThemeProvider onThemeChange={handleThemeChange}>
<AppShell title="ConusAI Browser">
	{#snippet sidebar()}
		<div class="shell-sidebar">
			<div class="sidebar-header">
				<span class="logo">ConusAI</span>
				{#if !shellReady}
					<span class="status-dot loading" title="Connecting…"></span>
				{:else}
					<span class="status-dot ready" title="Connected"></span>
				{/if}
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
			{activeTabId}
			onselect={(id) => (activeTabId = id)}
			onclose={handleCloseTab}
			oncreate={handleNewTab}
		/>
		<div class="page">
			{@render children()}
		</div>
	</div>
</AppShell>

<LiveAnnouncer />
</ThemeProvider>

<style>
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

	.status-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
	}

	.status-dot.loading {
		background: var(--ink-muted);
		animation: pulse 1.2s ease-in-out infinite;
	}

	.status-dot.ready {
		background: #22c55e;
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.3; }
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
