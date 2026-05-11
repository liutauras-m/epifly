<script lang="ts">
	import type { WorkspaceNode } from '@conusai/types';

	let {
		recentIds,
		nodes,
		onSelect,
	}: {
		recentIds: string[];
		nodes: WorkspaceNode[];
		onSelect: (n: WorkspaceNode) => void;
	} = $props();

	const recents = $derived(
		recentIds
			.map(id => nodes.find(n => n.id === id))
			.filter(Boolean) as WorkspaceNode[]
	);

	function timeAgo(ts: string | undefined): string {
		if (!ts) return '';
		const diff = (Date.now() - new Date(ts).getTime()) / 1000;
		if (diff < 60) return 'just now';
		if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
		if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
		return `${Math.floor(diff / 86400)}d ago`;
	}
</script>

{#if recents.length > 0}
	<section class="recents-section">
		<div class="section-header">
			<span class="section-label">RECENT</span>
		</div>
		{#each recents as node (node.id)}
			<button class="recent-row" onclick={() => onSelect(node)}>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="16" height="16" class="recent-icon">
					<path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/>
				</svg>
				<span class="recent-title">{node.name}</span>
				<span class="recent-time">{timeAgo((node as any).updated_at)}</span>
			</button>
		{/each}
	</section>
{/if}

<style>
	.recents-section {
		border-top: 1px solid var(--rule);
		display: flex;
		flex-direction: column;
	}

	.section-header {
		padding: var(--s-2) var(--s-3) var(--s-1) var(--s-4);
	}

	.section-label {
		font-family: var(--font-mono);
		font-size: 11px;
		font-weight: 500;
		letter-spacing: 0.08em;
		color: var(--ink-3);
		text-transform: uppercase;
	}

	.recent-row {
		display: flex;
		align-items: center;
		gap: var(--s-2);
		height: 44px;
		padding: 0 var(--s-4);
		border: none;
		background: none;
		cursor: pointer;
		width: 100%;
		text-align: left;
		transition: background 120ms;
	}

	.recent-row:hover { background: var(--paper-3); }

	.recent-icon { color: var(--ink-3); flex-shrink: 0; }

	.recent-title {
		flex: 1;
		font-family: var(--font-body);
		font-size: 15px;
		color: var(--ink);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.recent-time {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--ink-3);
		flex-shrink: 0;
	}
</style>
