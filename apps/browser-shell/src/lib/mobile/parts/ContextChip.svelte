<script lang="ts">
	import type { WorkspaceNode } from '@conusai/types';

	let {
		node,
		onClear,
	}: {
		node: WorkspaceNode;
		onClear: () => void;
	} = $props();
</script>

<div class="context-chip">
	<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="14" height="14">
		<path d="M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9z"/>
		<polyline points="13 2 13 9 20 9"/>
	</svg>
	<span class="chip-label">{node.name}</span>
	<button class="chip-clear" aria-label="Remove context" onclick={onClear}>
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="14" height="14">
			<path d="M18 6L6 18M6 6l12 12"/>
		</svg>
	</button>
</div>

<style>
	.context-chip {
		display: inline-flex;
		align-items: center;
		gap: var(--s-1);
		background: var(--ember-soft);
		border-radius: var(--r-md);
		padding: var(--s-1) var(--s-2);
		font-family: var(--font-mono);
		font-size: 12px;
		color: var(--ink-2);
	}

	.chip-label {
		max-width: 180px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.chip-clear {
		display: flex;
		align-items: center;
		background: none;
		border: none;
		padding: 0;
		cursor: pointer;
		color: var(--ink-3);
	}

	.chip-clear:hover { color: var(--ink); }
</style>
