<script lang="ts">
	import { goto } from '$app/navigation';
	import type { PageData } from './$types';
	import type { WorkspaceNode } from '@conusai/types';
	import { sdk } from '$lib/sdk';
	import { toasts } from '@conusai/ui/stores';
	import { provideCapabilityRendererRegistry } from '@conusai/ui/capabilities';
	import { AgentChatStream, AgentChatComposer, WorkspaceExplorer, createChatStream, type Attachment } from '@conusai/ui/features';
	import { ThemeSwitcher } from '@conusai/ui';
	import favicon from '@conusai/ui/assets/images/favicon.png';

	let { data }: { data: PageData } = $props();

	const registry = provideCapabilityRendererRegistry();
	const chatStream = createChatStream(sdk);

	let workspaceNodes = $state<WorkspaceNode[]>(data.workspaceTree ?? []);
	let selectedNodeId = $state<string | undefined>();
	let showChat = $state(false);
	let inputValue = $state('');
	let sidebarOpen = $state(false);
	const hour = new Date().getHours();
	const greeting = hour < 12 ? 'Good morning' : hour < 17 ? 'Good afternoon' : 'Good evening';
	let recents = $state<{ id: string; title: string }[]>([]);
	let messagesEl = $state<HTMLElement | undefined>();

	function onSelectNode(node: WorkspaceNode) {
		if (node.kind === 'conversation') {
			if (node.metadata?.thread_id) {
				chatStream.loadThread(node.metadata.thread_id as string);
				showChat = true;
			} else {
				chatStream.newSession();
				showChat = false;
			}
		}
		goto(`?ws=${node.id}`, { replaceState: true, keepFocus: true, noScroll: true });
	}

	function handleSubmit(prompt: string, attachments: Attachment[] = []) {
		if (!prompt.trim()) return;
		showChat = true;
		chatStream.send(prompt, {
			workspaceNodeId: selectedNodeId,
			attachmentIds: attachments.map(a => a.id),
			onThreadId(id) {
				recents = [{ id, title: prompt.slice(0, 60) }, ...recents.filter(r => r.id !== id)].slice(0, 20);
			},
		});
	}

	async function handleUpload(files: File[]) {
		const added: { id: string; filename: string; size: number }[] = [];
		for (const file of files) {
			const result = await sdk.workspaces.upload(file);
			if (result.error) { toasts.error(`Upload failed: ${result.error.message}`); continue; }
			if (result.data) added.push({ id: result.data.id, filename: result.data.filename, size: result.data.size });
		}
		return added;
	}

	function onKeydown(e: KeyboardEvent) {
		const mod = e.metaKey || e.ctrlKey;
		if (mod && e.key === 'n') { e.preventDefault(); chatStream.newSession(); showChat = false; }
	}
</script>

<svelte:window onkeydown={onKeydown} />
<svelte:head><title>Workshop · ConusAI</title></svelte:head>

<div class="app">
	<aside class="sidebar" class:open={sidebarOpen} aria-label="Workshop navigation">
		<WorkspaceExplorer {sdk} bind:nodes={workspaceNodes} bind:selectedNodeId {onSelectNode} />
		<div class="user-chip">
			<div class="avatar">{data.user?.initials ?? '?'}</div>
			<div class="user-meta">
				<span class="user-name">{data.user?.name ?? ''}</span>
				<span class="user-plan">{data.user?.plan ?? ''}</span>
			</div>
		</div>
	</aside>

	<main class="main">
		<div class="topbar">
			<button class="icon-btn" aria-label="Toggle nav" onclick={() => sidebarOpen = !sidebarOpen}>
				<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="square" width="18" height="18">
					<line x1="3" y1="5" x2="15" y2="5"/><line x1="3" y1="9" x2="15" y2="9"/><line x1="3" y1="13" x2="15" y2="13"/>
				</svg>
			</button>
			<div style="flex:1"></div>
			<ThemeSwitcher />
			<a href="/logout" class="icon-btn" aria-label="Logout" data-sveltekit-reload>
				<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" width="18" height="18">
					<path d="M7 3H3v12h4M12 6l4 3-4 3M6 9h10"/>
				</svg>
			</a>
		</div>

		{#if !showChat}
			<section class="greeting-screen">
				<div class="greeting">
					<img class="sigil" src={favicon} alt="" aria-hidden="true">
					<h1 class="greeting-text">{greeting}, {data.user?.firstName ?? 'there'}</h1>
				</div>
				<AgentChatComposer bind:value={inputValue} onsubmit={handleSubmit} onUpload={handleUpload} />
			</section>
		{:else}
			<section class="chat-view">
				<AgentChatStream
					messages={chatStream.messages}
					toolCards={chatStream.toolCards}
					inFlight={chatStream.inFlight}
					bind:messagesEl
				/>
				<div class="composer-bottom">
					<AgentChatComposer bind:value={inputValue} onsubmit={handleSubmit} onUpload={handleUpload} inFlight={chatStream.inFlight} />
				</div>
			</section>
		{/if}
	</main>
</div>

<style>
	.app { display: flex; height: 100dvh; overflow: hidden; background: var(--paper); }
	.sidebar {
		width: var(--rail, 240px); flex-shrink: 0; border-right: 1px solid var(--rule);
		display: flex; flex-direction: column; background: var(--paper-2); overflow: hidden;
	}
	.user-chip {
		display: flex; align-items: center; gap: var(--s-2);
		padding: var(--s-3) var(--s-4); border-top: 1px solid var(--rule); margin-top: auto;
	}
	.avatar {
		width: 28px; height: 28px; border-radius: 50%;
		background: var(--ember-soft); border: 1px solid var(--ember-glow);
		display: flex; align-items: center; justify-content: center;
		font-size: var(--t-label); font-weight: 600; color: var(--ember-2);
	}
	.user-meta { display: flex; flex-direction: column; }
	.user-name { font-size: var(--t-meta); color: var(--ink); }
	.user-plan { font-size: var(--t-label); color: var(--ink-3); }
	.main { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
	.topbar {
		display: flex; align-items: center; gap: var(--s-2);
		padding: 0 var(--s-4); height: 48px; border-bottom: 1px solid var(--rule);
		flex-shrink: 0;
	}
	.icon-btn {
		display: flex; align-items: center; justify-content: center;
		width: 32px; height: 32px; border-radius: var(--r-sm);
		background: none; border: none; cursor: pointer; color: var(--ink-3);
		text-decoration: none;
	}
	.icon-btn:hover { background: var(--paper-3); color: var(--ink); }
	.greeting-screen {
		flex: 1; display: flex; flex-direction: column;
		align-items: center; justify-content: center; gap: var(--s-6);
		padding: var(--s-8);
	}
	.greeting { display: flex; flex-direction: column; align-items: center; gap: var(--s-3); text-align: center; }
	.sigil { width: 48px; height: 48px; object-fit: contain; }
	.greeting-text { font-family: var(--font-display); font-size: var(--t-h1); color: var(--ink); margin: 0; }
	.chat-view { flex: 1; display: flex; flex-direction: column; overflow: hidden; }
	.composer-bottom { flex-shrink: 0; padding: var(--s-3) 0 var(--s-4); }
	@media (max-width: 640px) {
		.sidebar { position: fixed; inset: 0 auto 0 0; z-index: 100; transform: translateX(-100%); transition: transform var(--dur-3); }
		.sidebar.open { transform: translateX(0); box-shadow: 2px 0 24px var(--shadow-md); }
	}
</style>
