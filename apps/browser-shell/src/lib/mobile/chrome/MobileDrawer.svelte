<script lang="ts">
	import type { Snippet } from 'svelte';
	import { onMount } from 'svelte';

	let {
		open = false,
		onClose,
		children,
	}: {
		open?: boolean;
		onClose: () => void;
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

<!-- Backdrop -->
<div
	class="backdrop"
	class:visible={open}
	aria-hidden="true"
	onclick={onClose}
></div>

<!-- Panel -->
<nav
	class="drawer"
	class:open
	aria-label="Workspace navigation"
	aria-hidden={!open}
>
	<div class="drawer-inner">
		<div class="drawer-header">
			<button class="drawer-close" aria-label="Close" onclick={onClose}>
				<svg viewBox="0 0 24 24" fill="none" width="20" height="20" aria-hidden="true">
					<path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
				</svg>
			</button>
		</div>
		{@render children()}
	</div>
</nav>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.4);
		z-index: 300;
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--dur-2, 200ms) var(--ease-out, cubic-bezier(0.22, 1, 0.36, 1));
	}

	.backdrop.visible {
		opacity: 1;
		pointer-events: auto;
	}

	.drawer {
		position: fixed;
		top: 0;
		left: 0;
		bottom: 0;
		width: 84vw;
		max-width: 320px;
		background: var(--paper-2);
		border-right: 1px solid var(--seam);
		z-index: 310;
		transform: translateX(-100%);
		transition: transform var(--dur-3, 320ms) var(--ease-out, cubic-bezier(0.22, 1, 0.36, 1));
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.drawer.open {
		transform: translateX(0);
	}

	@media (prefers-reduced-motion: reduce) {
		.backdrop { transition-duration: 80ms; }
		.drawer { transition: opacity 80ms linear; transform: none !important; }
		.drawer:not(.open) { opacity: 0; pointer-events: none; }
		.drawer.open { opacity: 1; }
	}

	.drawer-inner {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow-y: auto;
		overflow-x: hidden;
		padding-top: env(safe-area-inset-top);
		padding-bottom: env(safe-area-inset-bottom);
	}

	.drawer-header {
		display: flex;
		justify-content: flex-end;
		padding: var(--s-2) var(--s-3) 0;
		flex-shrink: 0;
	}

	.drawer-close {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		border-radius: 50%;
		border: none;
		background: transparent;
		color: var(--ink-3);
		cursor: pointer;
		padding: 0;
	}

	.drawer-close:hover {
		background: var(--paper-3);
		color: var(--ink);
	}
</style>
