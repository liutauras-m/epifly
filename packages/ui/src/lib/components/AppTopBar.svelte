<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		onMenuToggle,
		canGoBack = false,
		onBack,
		title = 'ConusAI',
		rightAction,
	}: {
		onMenuToggle: () => void;
		canGoBack?: boolean;
		onBack?: () => void;
		title?: string;
		/** Slot for trailing actions (new chat, capabilities toggle, theme, logout). */
		rightAction?: Snippet;
	} = $props();
</script>

<!--
  Two-layer structure:
  - .topbar fills the safe-area inset with the background colour (no content there).
  - .topbar-inner holds the actual 48 px row of buttons and title.
  This prevents the Dynamic Island from squishing button targets.
-->
<header class="topbar">
	<div class="topbar-inner">
		<button
			class="topbar-btn"
			aria-label={canGoBack ? 'Go back' : 'Toggle navigation'}
			onclick={canGoBack ? onBack : onMenuToggle}
		>
			{#if canGoBack}
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
					stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
					width="22" height="22" aria-hidden="true">
					<path d="M15 18l-6-6 6-6"/>
				</svg>
			{:else}
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
					stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
					width="22" height="22" aria-hidden="true">
					<line x1="3" y1="6" x2="21" y2="6"/>
					<line x1="3" y1="12" x2="21" y2="12"/>
					<line x1="3" y1="18" x2="21" y2="18"/>
				</svg>
			{/if}
		</button>

		<span class="topbar-title">{title}</span>

		<div class="topbar-right">
			{#if rightAction}
				{@render rightAction()}
			{/if}
		</div>
	</div>
</header>

<style>
	/* @deprecated — use AppHeader (Phase 3.3). Shim deleted at Phase 4 close. */
	.topbar {
		padding-top: var(--safe-top, env(safe-area-inset-top, 0px));
		background: var(--color-bg);
		border-bottom: 1px solid var(--color-border);
		flex-shrink: 0;
		z-index: var(--z-topbar, 100);
	}

	.topbar-inner {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		height: 48px;
		padding: 0 var(--space-2);
	}

	.topbar-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 44px;
		height: 44px;
		border: none;
		background: none;
		color: var(--color-fg);
		cursor: pointer;
		border-radius: var(--radius-sm);
		flex-shrink: 0;
	}
	.topbar-btn:hover { background: var(--color-bg-hover); }
	.topbar-btn:focus-visible {
		outline: var(--focus-ring);
		outline-offset: var(--focus-ring-offset);
	}

	.topbar-title {
		flex: 1;
		font-family: var(--font-family-sans);
		font-size: var(--font-size-h2);
		font-weight: 600;
		letter-spacing: -0.4px;
		color: var(--color-fg);
		text-align: center;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.topbar-right {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--space-1);
		min-width: 44px;
		height: 44px;
		flex-shrink: 0;
	}

	/* Desktop: smaller buttons, left-aligned title */
	@container app-shell (min-width: 641px) {
		.topbar-inner { padding: 0 var(--space-3); }
		.topbar-btn { width: 36px; height: 36px; }
		.topbar-title { text-align: left; font-size: var(--font-size-body); font-weight: 500; }
	}
</style>
