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
	aria-label="Navigation drawer"
	aria-hidden={!open}
>
	<div class="drawer-inner">
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
</style>
