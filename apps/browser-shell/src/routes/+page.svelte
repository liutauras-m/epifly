<script lang="ts">
	import { sdk } from '$lib/sdk';
	import { invoke } from '@tauri-apps/api/core';
	import { provideCapabilityRendererRegistry } from '@conusai/ui/capabilities';
	import { AgentChatStream, AgentChatComposer, WorkspaceExplorer, LoginPanel, createChatStream } from '@conusai/ui/features';
	import { modeStore } from '@conusai/ui/stores';
	import TraceReplayCapability from '$lib/TraceReplayCapability.svelte';
	import type { WorkspaceNode } from '@conusai/types';

	modeStore.setMode('shell');

	const registry = provideCapabilityRendererRegistry();
	// Register shell-local capability renderers. The backend registers trace.replay
	// as a remote-MCP capability; this renderer is invoked when the agent uses it.
	registry.register('trace.replay', TraceReplayCapability as any);
	const chatStream = createChatStream(sdk);

	let authenticated = $state(false);
	let showChat = $state(false);
	let inputValue = $state('');
	let workspaceNodes = $state<WorkspaceNode[]>([]);
	let selectedNodeId = $state<string | undefined>();

	async function handleAuthenticated(token: string) {
		await invoke('set_device_token', { token });
		// Persist to Stronghold via JS bridge.
		try {
			const { Client } = await import('@tauri-apps/plugin-stronghold');
			const { appDataDir } = await import('@tauri-apps/api/path');
			const vaultPath = (await appDataDir()) + '/conusai.stronghold';
			const client = await Client.load(vaultPath, 'conusai-shell-v1');
			const store = client.getStore('tokens');
			await store.insert('device_token', Array.from(new TextEncoder().encode(token)));
		} catch {
			// Stronghold not available — token lives in Rust state for this session.
		}
		authenticated = true;
	}

	function handleSelectNode(node: WorkspaceNode) {
		showChat = true;
		selectedNodeId = node.id;
		if (node.kind === 'conversation' && node.metadata?.thread_id) {
			chatStream.loadThread(node.metadata.thread_id as string);
		}
	}

	function handleSubmit(prompt: string) {
		if (!prompt.trim()) return;
		showChat = true;
		chatStream.send(prompt, { workspaceNodeId: selectedNodeId });
	}
</script>

{#if !authenticated}
	<LoginPanel {sdk} onAuthenticated={handleAuthenticated} />
{:else}
	<div class="shell-workspace">
		<aside class="shell-nav">
			<WorkspaceExplorer {sdk} bind:nodes={workspaceNodes} bind:selectedNodeId onSelectNode={handleSelectNode} />
		</aside>

		<main class="shell-main">
			{#if showChat}
				<AgentChatStream
					messages={chatStream.messages}
					toolCards={chatStream.toolCards}
					inFlight={chatStream.inFlight}
				/>
				<div class="composer-wrap">
					<AgentChatComposer bind:value={inputValue} onsubmit={handleSubmit} inFlight={chatStream.inFlight} />
				</div>
			{:else}
				<div class="empty-state">
					<p>Select a conversation or start a new one.</p>
					<AgentChatComposer bind:value={inputValue} onsubmit={handleSubmit} />
				</div>
			{/if}
		</main>
	</div>
{/if}

<style>
	.shell-workspace {
		display: flex;
		height: 100%;
		overflow: hidden;
	}

	.shell-nav {
		width: 220px;
		flex-shrink: 0;
		border-right: 1px solid var(--rule);
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}

	.shell-main {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.composer-wrap {
		flex-shrink: 0;
		padding: var(--s-3) 0 var(--s-4);
	}

	.empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--s-6);
		padding: var(--s-8);
		color: var(--ink-3);
	}
</style>
