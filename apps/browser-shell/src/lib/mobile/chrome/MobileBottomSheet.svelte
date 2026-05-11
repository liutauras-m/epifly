<script lang="ts">
	import type { Snippet } from 'svelte';
	import { onMount } from 'svelte';

	let {
		open = false,
		onClose,
		title,
		children,
	}: {
		open?: boolean;
		onClose: () => void;
		title?: string;
		children: Snippet;
	} = $props();

	onMount(() => {
		function onKey(e: KeyboardEvent) {
			if (e.key === 'Escape' && open) { e.preventDefault(); onClose(); }
		}
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	});
</script>

<div
	class="backdrop"
	class:visible={open}
	aria-hidden="true"
	onclick={onClose}
></div>

<div
	class="sheet"
	class:open
	role="dialog"
	aria-modal="true"
	aria-label={title ?? 'Options'}
>
	<div class="drag-handle" aria-hidden="true"></div>

	{#if title}
		<div class="sheet-header">
			<span class="sheet-title">{title}</span>
			<button class="sheet-close" aria-label="Close" onclick={onClose}>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="24" height="24">
					<path d="M18 6L6 18M6 6l12 12"/>
				</svg>
			</button>
		</div>
	{/if}

	<div class="sheet-body">
		{@render children()}
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.4);
		z-index: 400;
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--dur-2, 200ms) var(--ease-out, cubic-bezier(0.22, 1, 0.36, 1));
	}

	.backdrop.visible {
		opacity: 1;
		pointer-events: auto;
	}

	.sheet {
		position: fixed;
		bottom: 0;
		left: 0;
		right: 0;
		background: var(--paper);
		border-radius: var(--r-lg, 20px) var(--r-lg, 20px) 0 0;
		border-top: 1px solid var(--rule);
		z-index: 410;
		transform: translateY(100%);
		transition: transform var(--dur-3, 320ms) var(--ease-out, cubic-bezier(0.22, 1, 0.36, 1));
		max-height: 90vh;
		display: flex;
		flex-direction: column;
		padding-bottom: env(safe-area-inset-bottom);
	}

	.sheet.open {
		transform: translateY(0);
	}

	.drag-handle {
		width: 40px;
		height: 4px;
		background: var(--rule);
		border-radius: 2px;
		margin: var(--s-2) auto var(--s-1);
		flex-shrink: 0;
	}

	.sheet-header {
		display: flex;
		align-items: center;
		padding: var(--s-2) var(--s-4) var(--s-2) var(--s-4);
		border-bottom: 1px solid var(--rule);
		flex-shrink: 0;
	}

	.sheet-title {
		flex: 1;
		font-family: var(--font-display);
		font-size: 16px;
		font-weight: 600;
		color: var(--ink);
	}

	.sheet-close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 44px;
		height: 44px;
		border: none;
		background: none;
		color: var(--ink-3);
		cursor: pointer;
		border-radius: var(--r-sm);
	}

	.sheet-close:hover { background: var(--paper-2); }

	.sheet-body {
		flex: 1;
		overflow-y: auto;
	}
</style>
