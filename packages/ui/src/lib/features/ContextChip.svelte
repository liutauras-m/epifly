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

<button
	class="context-chip"
	onclick={onClear}
	aria-label="Clear context: {node.name}"
>
	{#if node.kind === 'folder'}
		<!-- Folder icon -->
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
			stroke-linecap="round" stroke-linejoin="round" width="14" height="14" aria-hidden="true">
			<path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"/>
		</svg>
	{:else}
		<!-- File / conversation icon -->
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
			stroke-linecap="round" stroke-linejoin="round" width="14" height="14" aria-hidden="true">
			<path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/>
		</svg>
	{/if}

	<span class="chip-label">{node.name}</span>

	<!-- Dismiss × -->
	<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
		stroke-linecap="round" stroke-linejoin="round" width="12" height="12"
		class="chip-x" aria-hidden="true">
		<path d="M18 6L6 18M6 6l12 12"/>
	</svg>
</button>

<style>
	.context-chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		padding: 4px var(--space-2) 4px var(--space-2);
		background: var(--ember-soft);
		border: 1px solid var(--ember-glow);
		border-radius: var(--radius-full);
		cursor: pointer;
		font-family: var(--font-mono);
		font-size: 12px;
		color: var(--ink-2);
		transition: background var(--duration-fast), border-color var(--duration-fast);
	}

	.context-chip:hover {
		background: var(--paper-3);
		border-color: var(--rule);
	}

	.context-chip:focus-visible {
		outline: 2px solid var(--ember);
		outline-offset: 2px;
	}

	.chip-label {
		max-width: 180px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.chip-x {
		color: var(--ink-3);
		flex-shrink: 0;
		margin-left: 2px;
	}

	@media (prefers-reduced-motion: reduce) {
		.context-chip { transition: none; }
	}
</style>
