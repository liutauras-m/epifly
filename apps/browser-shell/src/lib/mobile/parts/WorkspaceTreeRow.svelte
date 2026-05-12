<script lang="ts">
	import type { WorkspaceNode } from '@conusai/types';

	let {
		node,
		depth = 0,
		active = false,
		onSelect,
		onCreateChild,
	}: {
		node: WorkspaceNode;
		depth?: number;
		active?: boolean;
		onSelect: (n: WorkspaceNode) => void;
		onCreateChild?: (parentId: string) => void;
	} = $props();

	let expanded = $state(false);
	const hasChildren = $derived((node as any).children?.length > 0 || node.kind === 'folder');
	const children: WorkspaceNode[] = $derived((node as any).children ?? []);

	function getIcon(kind: string) {
		if (kind === 'folder') return 'folder';
		if (kind === 'conversation') return 'chat';
		return 'file';
	}
</script>

<div class="tree-item">
	<button
		class="row"
		class:active
		style="padding-left: calc(var(--s-4) + {depth * 16}px)"
		onclick={() => {
			if (node.kind === 'folder') expanded = !expanded;
			else onSelect(node);
		}}
		aria-expanded={node.kind === 'folder' ? expanded : undefined}
	>
		{#if node.kind === 'folder'}
			<svg class="chevron" class:rotated={expanded} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="16" height="16">
				<path d="M9 18l6-6-6-6"/>
			</svg>
		{:else}
			<span class="icon-space"></span>
		{/if}

		{#if getIcon(node.kind) === 'folder'}
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="18" height="18" class="node-icon">
				<path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"/>
			</svg>
		{:else if getIcon(node.kind) === 'chat'}
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="18" height="18" class="node-icon">
				<path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/>
			</svg>
		{:else}
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="18" height="18" class="node-icon">
				<path d="M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9z"/>
				<polyline points="13 2 13 9 20 9"/>
			</svg>
		{/if}

		<span class="label">{node.name}</span>

		{#if active}
			<span class="active-rail" aria-hidden="true"></span>
		{/if}
	</button>

	{#if expanded && children.length > 0}
		<div class="children">
			{#each children as child (child.id)}
				<WorkspaceTreeRow
					node={child}
					depth={depth + 1}
					{onSelect}
					{onCreateChild}
				/>
			{/each}
		</div>
	{/if}
</div>

<style>
	.row {
		display: flex;
		align-items: center;
		width: 100%;
		height: 44px;
		border: none;
		background: none;
		cursor: pointer;
		gap: var(--s-2);
		padding-right: var(--s-3);
		color: var(--ink);
		font-family: var(--font-body);
		font-size: 15px;
		text-align: left;
		position: relative;
		transition: background var(--dur-1);
	}

	.row:hover { background: var(--paper-3); }

	.row.active {
		background: var(--ember-soft);
	}

	.active-rail {
		position: absolute;
		left: 0;
		top: 8px;
		bottom: 8px;
		width: 2px;
		background: var(--ember);
		border-radius: 999px;
		animation: rail-in var(--dur-2) var(--ease-out) forwards;
		transform: scaleY(0);
		transform-origin: top;
	}

	@keyframes rail-in {
		to { transform: scaleY(1); }
	}

	.chevron {
		flex-shrink: 0;
		color: var(--ink-3);
		transition: transform var(--dur-2) var(--ease-out);
	}

	.chevron.rotated { transform: rotate(90deg); }

	.icon-space { width: 16px; flex-shrink: 0; }

	.node-icon {
		flex-shrink: 0;
		color: var(--ink-3);
	}

	.label {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	@media (prefers-reduced-motion: reduce) {
		.row { transition: none; }
		.active-rail { animation: none; transform: scaleY(1); }
		.chevron { transition: none; }
	}
</style>
