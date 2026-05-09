<!--
  WorkspaceTree — top-level workspace component.

  Owns all workspace state via the runes context store (context.svelte.ts).
  Props:
    - nodes: initial tree from SSR (passed from +page.svelte data)
    - onSelectConversation: called when a conversation node is activated;
      the parent (page) wires this to loadThread(threadId).

  All API calls go through src/lib/api/workspaces.ts.
  No prompt() / confirm() — dialogs are Svelte components.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { workspacesApi } from '$lib/api';
	import { toasts } from '$lib/ui/toast.svelte';
	import { createWorkspaceContext, provideWorkspaceContext } from './context.svelte';
	import NewNodeDialog from './dialogs/NewNodeDialog.svelte';
	import ConfirmDialog from './dialogs/ConfirmDialog.svelte';
	import MoveDialog from './dialogs/MoveDialog.svelte';
	import ShareDialog from './dialogs/ShareDialog.svelte';
	import type { WorkspaceNode } from '$lib/types';

	let { nodes, onSelectConversation }: {
		nodes: WorkspaceNode[];
		onSelectConversation: (node: WorkspaceNode, threadId: string | null) => void;
	} = $props();

	// ── Context ───────────────────────────────────────────────────────────────
	const ctx = createWorkspaceContext();
	provideWorkspaceContext(ctx);
	ctx.setTree(nodes);

	// ── Dialog state ──────────────────────────────────────────────────────────
	let newNodeParent = $state<{ id: string | null; name?: string } | null>(null);
	let confirmNode = $state<WorkspaceNode | null>(null);
	let moveNode = $state<WorkspaceNode | null>(null);
	let shareNode = $state<(WorkspaceNode & { shared_with?: string[] }) | null>(null);

	// ── URL deep-link restore ─────────────────────────────────────────────────
	onMount(() => {
		const wsId = $page.url.searchParams.get('ws');
		if (wsId) ctx.setSelected(wsId);
	});

	// ── Tree operations ───────────────────────────────────────────────────────
	async function expandFolder(node: WorkspaceNode) {
		if (ctx.expanded.has(node.id)) { ctx.toggleExpanded(node.id); return; }
		ctx.toggleExpanded(node.id);
		if (!ctx.childMap.has(node.id)) {
			const result = await workspacesApi.getTree(fetch, node.id);
			if (result.error) { toasts.error(`Failed to load folder: ${result.error.message}`); return; }
			const raw = result.data;
			ctx.setChildren(node.id, Array.isArray(raw) ? raw : ((raw as { nodes?: WorkspaceNode[] })?.nodes ?? []));
		}
	}

	async function selectConversation(node: WorkspaceNode) {
		ctx.setSelected(node.id);
		goto(`?ws=${node.id}`, { replaceState: true, keepFocus: true, noScroll: true });
		let full = node;
		if (!full.metadata) {
			const r = await workspacesApi.getNode(fetch, node.id);
			if (!r.error) full = r.data;
		}
		onSelectConversation(full, full.metadata?.thread_id ?? null);
	}

	async function createNode(kind: 'folder' | 'conversation', name: string) {
		const parentId = newNodeParent?.id ?? null;
		const result = await workspacesApi.createNode(fetch, { kind, name, parent_id: parentId });
		newNodeParent = null;
		if (result.error) { toasts.error(`Create failed: ${result.error.message}`); return; }
		await refreshTree();
	}

	async function doDelete(node: WorkspaceNode) {
		confirmNode = null;
		const result = await workspacesApi.deleteNode(fetch, node.id);
		if (result.error) { toasts.error(`Delete failed: ${result.error.message}`); return; }
		toasts.success(`"${node.name}" deleted`);
		if (ctx.selectedId === node.id) {
			ctx.setSelected(null);
			goto('?', { replaceState: true, keepFocus: true, noScroll: true });
		}
		await refreshTree();
	}

	async function doMove(node: WorkspaceNode, newParentPath: string | null) {
		moveNode = null;
		const result = await workspacesApi.moveNode(fetch, node.id, { new_parent_id: null, new_parent_path: newParentPath });
		if (result.error) { toasts.error(`Move failed: ${result.error.message}`); return; }
		toasts.success('Moved successfully');
		await refreshTree();
	}

	async function refreshTree() {
		const result = await workspacesApi.getTree(fetch);
		if (!result.error) {
			const raw = result.data;
			ctx.setTree(Array.isArray(raw) ? raw : ((raw as { nodes?: WorkspaceNode[] })?.nodes ?? []));
		}
	}

	// ── Search ────────────────────────────────────────────────────────────────
	let searchTimer: ReturnType<typeof setTimeout> | null = null;

	function onSearchInput(e: Event) {
		const q = (e.target as HTMLInputElement).value;
		if (searchTimer) clearTimeout(searchTimer);
		if (!q.trim()) { ctx.setSearch('', []); return; }
		ctx.setSearch(q, ctx.searchResults); // keep stale results while loading
		searchTimer = setTimeout(async () => {
			const result = await workspacesApi.searchNodes(fetch, q.trim());
			if (!result.error) {
				const raw = result.data;
				ctx.setSearch(q, Array.isArray(raw) ? raw : ((raw as { nodes?: WorkspaceNode[] })?.nodes ?? []));
			}
		}, 220);
	}
</script>

<!-- ── Search bar ── -->
<div class="ws-search-wrap">
	<svg class="ws-search-icon" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
		<circle cx="6.5" cy="6.5" r="4.5"/><line x1="10.5" y1="10.5" x2="14" y2="14"/>
	</svg>
	<input id="ws-search" class="ws-search-input" type="search" placeholder="Search conversations…"
		autocomplete="off" spellcheck="false" aria-label="Search workspace"
		value={ctx.searchQuery} oninput={onSearchInput}>
	{#if ctx.searchQuery}
		<button class="ws-search-clear" aria-label="Clear search" onclick={() => ctx.setSearch('', [])}>
			<svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
				<line x1="2" y1="2" x2="10" y2="10"/><line x1="10" y1="2" x2="2" y2="10"/>
			</svg>
		</button>
	{/if}
</div>

<!-- ── Tree / search results ── -->
{#if ctx.searchQuery}
	<div class="ws-tree" role="listbox" aria-label="Search results">
		{#if ctx.searchResults.length === 0}
			<div class="empty-hint">No matches for "{ctx.searchQuery}"</div>
		{:else}
			{#each ctx.searchResults as node (node.id)}
				<button class="ws-node ws-node-{node.kind}" class:ws-node-selected={ctx.selectedId === node.id}
					role="option" aria-selected={ctx.selectedId === node.id}
					onclick={() => node.kind === 'conversation' && selectConversation(node)}>
					<span class="ws-node-icon">{node.kind === 'folder' ? '📁' : '📄'}</span>
					<span class="ws-node-name">{node.name}</span>
					<span class="ws-node-path">{node.virtual_path}</span>
				</button>
			{/each}
		{/if}
	</div>
{:else}
	<div id="workspace-tree" class="ws-tree" role="tree" aria-label="Workspace">
		{#if ctx.tree.length === 0}
			<div class="empty-hint">No folders yet — click <strong>+</strong> to create one.</div>
		{:else}
			{#each ctx.tree as node (node.id)}
				{@render treeNode(node, 0)}
			{/each}
		{/if}
	</div>
{/if}

<!-- ── Dialogs ── -->
{#if newNodeParent !== null}
	<NewNodeDialog
		parentName={newNodeParent?.name}
		onsubmit={createNode}
		oncancel={() => (newNodeParent = null)}
	/>
{/if}

{#if confirmNode}
	<ConfirmDialog
		message={`Delete "${confirmNode.name}"? This cannot be undone.`}
		onconfirm={() => confirmNode && doDelete(confirmNode)}
		oncancel={() => (confirmNode = null)}
	/>
{/if}

{#if moveNode}
	<MoveDialog
		nodeName={moveNode.name}
		onmove={(path) => moveNode && doMove(moveNode, path)}
		oncancel={() => (moveNode = null)}
	/>
{/if}

{#if shareNode}
	<ShareDialog
		node={shareNode}
		onclose={() => (shareNode = null)}
	/>
{/if}

<!-- ── Tree node snippet ── -->
{#snippet treeNode(node: WorkspaceNode, depth: number)}
	{#if node.kind === 'folder'}
		<div class="ws-folder" style="--depth:{depth}">
			<button class="ws-node ws-node-folder"
				class:ws-node-expanded={ctx.expanded.has(node.id)}
				class:ws-node-selected={ctx.selectedId === node.id}
				onclick={() => { ctx.setSelected(node.id); goto(`?ws=${node.id}`, { replaceState: true, keepFocus: true, noScroll: true }); expandFolder(node); }}
				aria-expanded={ctx.expanded.has(node.id)}
				oncontextmenu={(e) => { e.preventDefault(); confirmNode = null; /* future context menu */ }}>
				<span class="ws-node-chevron">{ctx.expanded.has(node.id) ? '▾' : '▸'}</span>
				<span class="ws-node-icon">📁</span>
				<span class="ws-node-name">{node.name}</span>
			</button>
			{#if ctx.expanded.has(node.id)}
				<div class="ws-children">
					{#if ctx.childMap.has(node.id)}
						{#each ctx.childMap.get(node.id) ?? [] as child (child.id)}
							{@render treeNode(child, depth + 1)}
						{/each}
					{:else}
						<div class="ws-loading">Loading…</div>
					{/if}
				</div>
			{/if}
		</div>
	{:else}
		<button class="ws-node ws-node-conversation"
			class:ws-node-selected={ctx.selectedId === node.id}
			style="--depth:{depth}"
			onclick={() => selectConversation(node)}
			onkeydown={(e) => e.key === 'Enter' && selectConversation(node)}>
			<span class="ws-node-icon">📄</span>
			<span class="ws-node-name">{node.name}</span>
		</button>
	{/if}
{/snippet}
