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
	onkeydown={(e) => { if (e.key === 'Escape') onClose(); }}
	role="presentation"
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
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
					stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
					width="22" height="22" aria-hidden="true">
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
		background: var(--backdrop);
		z-index: 400;
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--duration-normal) var(--ease-out);
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
		border-radius: var(--radius-lg) var(--radius-lg) 0 0;
		border-top: 1px solid var(--rule);
		z-index: 410;
		transform: translateY(100%);
		transition: transform var(--duration-slow) var(--ease-out);
		max-height: 90vh;
		display: flex;
		flex-direction: column;
		padding-bottom: env(safe-area-inset-bottom);
	}
	.sheet.open { transform: translateY(0); }

	/* Desktop: center modal instead of bottom sheet */
	@media (min-width: 641px) {
		.sheet {
			top: 50%;
			left: 50%;
			right: auto;
			bottom: auto;
			width: min(560px, 92vw);
			max-height: min(80vh, 720px);
			border-radius: var(--radius-lg);
			border: 1px solid var(--rule);
			transform: translate(-50%, -50%) scale(0.96);
			opacity: 0;
		}
		.sheet.open {
			transform: translate(-50%, -50%) scale(1);
			opacity: 1;
		}
	}

	.drag-handle {
		width: 40px;
		height: 4px;
		background: var(--rule);
		border-radius: 999px;
		margin: var(--space-2) auto var(--space-1);
		flex-shrink: 0;
	}

	@media (min-width: 641px) {
		.drag-handle { display: none; }
	}

	.sheet-header {
		display: flex;
		align-items: center;
		padding: var(--space-2) var(--space-4);
		border-bottom: 1px solid var(--rule);
		flex-shrink: 0;
	}

	.sheet-title {
		flex: 1;
		font-family: var(--font-family-sans);
		font-size: var(--font-size-h2);
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
		border-radius: var(--radius-sm);
	}
	.sheet-close:hover { background: var(--paper-2); }

	.sheet-body {
		flex: 1;
		overflow-y: auto;
	}

	@media (prefers-reduced-motion: reduce) {
		.backdrop { transition: opacity 0.01ms; }
		.sheet { transition: none; }
	}
</style>
