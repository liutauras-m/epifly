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

<!-- Backdrop (mobile only — desktop has persistent sidebar) -->
<div
	class="backdrop"
	class:visible={open}
	aria-hidden="true"
	onclick={onClose}
	onkeydown={(e) => { if (e.key === 'Escape') onClose(); }}
	role="presentation"
></div>

<!-- Drawer / sidebar panel -->
<nav
	class="drawer"
	class:open
	aria-label="Workspace navigation"
>
	<div class="drawer-inner">
		<!-- Close button (mobile only — hidden on desktop via CSS) -->
		<div class="drawer-header">
			<button class="drawer-close" aria-label="Close navigation" onclick={onClose}>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
					stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
					width="20" height="20" aria-hidden="true">
					<path d="M18 6L6 18M6 6l12 12"/>
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
		background: var(--backdrop);
		z-index: 300;
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--dur-2) var(--ease-out);
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
		transition: transform var(--dur-3) var(--ease-out);
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}
	.drawer.open {
		transform: translateX(0);
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
		border-radius: var(--r-full);
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

	/* ── Desktop: persistent sidebar, no overlay ────────────────────────── */
	@media (min-width: 641px) {
		.backdrop { display: none; }

		.drawer {
			position: relative;
			top: auto;
			left: auto;
			bottom: auto;
			width: var(--rail, 240px);
			max-width: var(--rail, 240px);
			transform: none;
			transition: none;
			z-index: 1;
			flex-shrink: 0;
		}

		.drawer-header {
			display: none;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.backdrop { transition-duration: 80ms; }
		.drawer { transition: opacity 80ms linear; transform: none !important; }
		.drawer:not(.open) { opacity: 0; pointer-events: none; }
		.drawer.open { opacity: 1; }
	}

	@media (prefers-reduced-motion: reduce) and (min-width: 641px) {
		.drawer, .drawer:not(.open), .drawer.open { opacity: 1; pointer-events: auto; }
	}
</style>
