<script lang="ts">
	import type { ConusSdk } from '@conusai/sdk';
	import type { WorkspaceNode } from '@conusai/types';
	import WorkspaceTreeRow from './WorkspaceTreeRow.svelte';
	import WorkspaceCreateMenu from './WorkspaceCreateMenu.svelte';

	let {
		sdk,
		selectedNodeId,
		onSelectNode,
		onNodesLoaded,
		refreshSignal = 0,
	}: {
		sdk: ConusSdk;
		selectedNodeId?: string;
		onSelectNode: (n: WorkspaceNode) => void;
		/** Called after the root-level tree is fetched, with the loaded nodes. */
		onNodesLoaded?: (nodes: WorkspaceNode[]) => void;
		/**
		 * Increment this counter whenever an external event (e.g. a
		 * `resource_invalidated` SSE delta) means the tree should re-fetch.
		 * The `$effect` below tracks it and calls `loadTree()` on every change.
		 */
		refreshSignal?: number;
	} = $props();

	let nodes = $state<WorkspaceNode[]>([]);
	let loading = $state(true);
	let showCreateMenu = $state(false);
	let newFolderName = $state('');
	let creatingFolder = $state(false);
	let error = $state('');

	$effect(() => {
		// Track refreshSignal so an external increment triggers a re-fetch
		// (e.g. from a `resource_invalidated` SSE event in MobileShell).
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		refreshSignal;
		loadTree();
	});

	async function loadTree() {
		loading = true;
		const res = await sdk.workspaces.tree();
		if (!res.error && res.data) {
			nodes = (res.data as any).nodes ?? (Array.isArray(res.data) ? res.data : []);
			onNodesLoaded?.(nodes);
		}
		loading = false;
	}

	async function createFolder() {
		const name = newFolderName.trim();
		if (!name) return;
		creatingFolder = true;
		const res = await sdk.workspaces.create({ kind: 'folder', name });
		if (!res.error) {
			newFolderName = '';
			await loadTree();
		} else {
			error = 'Failed to create folder';
		}
		creatingFolder = false;
	}

	async function createConversation() {
		const res = await sdk.workspaces.create({ kind: 'conversation', name: 'New chat' });
		if (!res.error && res.data) {
			await loadTree();
			onSelectNode((res.data as any) as WorkspaceNode);
		}
	}

	/** Lazy-loads children of a folder on first expand. */
	async function loadChildren(parentId: string): Promise<WorkspaceNode[]> {
		const res = await sdk.workspaces.tree(parentId);
		if (res.error || !res.data) return [];
		const d = res.data as any;
		return Array.isArray(d) ? d : (d.nodes ?? []);
	}

	let pendingFolder = $state(false);
</script>

<section class="ws-section">
	<div class="ws-header">
		<span class="section-label">WORKSPACE</span>
		<div class="ws-header-actions">
			<button
				class="icon-btn"
				aria-label="New folder or conversation"
				onclick={() => showCreateMenu = !showCreateMenu}
			>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="20" height="20">
					<line x1="12" y1="5" x2="12" y2="19"/>
					<line x1="5" y1="12" x2="19" y2="12"/>
				</svg>
			</button>
			{#if showCreateMenu}
				<WorkspaceCreateMenu
					onNewFolder={() => { pendingFolder = true; showCreateMenu = false; }}
					onNewConversation={createConversation}
					onClose={() => showCreateMenu = false}
				/>
			{/if}
		</div>
	</div>

	{#if pendingFolder}
		<div class="new-folder-row">
			<input
				class="folder-input"
				type="text"
				placeholder="Folder name"
				bind:value={newFolderName}
				autofocus
				onkeydown={(e) => {
					if (e.key === 'Enter') { createFolder(); pendingFolder = false; }
					if (e.key === 'Escape') { pendingFolder = false; newFolderName = ''; }
				}}
			/>
			<button class="confirm-btn" onclick={() => { createFolder(); pendingFolder = false; }} disabled={creatingFolder}>
				{creatingFolder ? '...' : 'Create'}
			</button>
		</div>
	{/if}

	{#if loading}
		<div class="skeleton-list">
			{#each [1, 2, 3, 4] as _}
				<div class="skeleton-row"></div>
			{/each}
		</div>
	{:else if nodes.length === 0 && !pendingFolder}
		<p class="empty">No folders yet — tap <strong>+</strong> to create one.</p>
	{:else}
		<div class="tree" role="tree" aria-label="Workspace tree">
			{#each nodes as node (node.id)}
				<WorkspaceTreeRow
					{node}
					active={node.id === selectedNodeId}
					onSelect={(n) => onSelectNode(n)}
					{loadChildren}
				/>
			{/each}
		</div>
	{/if}

	{#if error}
		<p class="error-msg">{error}</p>
	{/if}
</section>

<style>
	.ws-section {
		display: flex;
		flex-direction: column;
	}

	.ws-header {
		display: flex;
		align-items: center;
		padding: var(--space-2) var(--space-3) var(--space-2) var(--space-4);
		position: relative;
	}

	.section-label {
		flex: 1;
		font-family: var(--font-mono);
		font-size: 11px;
		font-weight: 500;
		letter-spacing: 0.14em;
		color: var(--ink-3);
		text-transform: uppercase;
	}

	.ws-header-actions {
		position: relative;
	}

	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		border: none;
		background: none;
		color: var(--ink-3);
		cursor: pointer;
		border-radius: var(--radius-sm);
	}

	.icon-btn:hover { background: var(--paper-3); color: var(--ink); }

	.new-folder-row {
		display: flex;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-4);
	}

	.folder-input {
		flex: 1;
		height: 36px;
		border: 1px solid var(--rule);
		border-radius: var(--radius-sm);
		padding: 0 var(--space-3);
		background: var(--paper);
		color: var(--ink);
		font-family: var(--font-family-sans);
		font-size: 14px;
	}

	.folder-input:focus {
		outline: none;
		border-color: var(--ember);
	}

	.confirm-btn {
		height: 36px;
		padding: 0 var(--space-3);
		border: none;
		background: var(--ember);
		color: var(--ink);
		border-radius: var(--radius-sm);
		font-family: var(--font-family-sans);
		font-size: 14px;
		cursor: pointer;
	}

	.empty {
		padding: var(--space-3) var(--space-4);
		font-family: var(--font-family-sans);
		font-size: 14px;
		color: var(--ink-3);
	}

	.tree {
		display: flex;
		flex-direction: column;
	}

	.skeleton-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		padding: var(--space-2) var(--space-4);
	}

	.skeleton-row {
		height: 36px;
		background: var(--paper-2);
		border-radius: var(--radius-sm);
		animation: shimmer 1.2s ease-in-out infinite;
	}

	@keyframes shimmer {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.5; }
	}

	@media (prefers-reduced-motion: reduce) {
		.skeleton-row { animation: none; }
	}

	.error-msg {
		padding: var(--space-2) var(--space-4);
		font-family: var(--font-family-sans);
		font-size: 13px;
		color: var(--danger);
	}
</style>
