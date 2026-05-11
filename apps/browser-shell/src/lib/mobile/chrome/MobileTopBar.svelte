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
		rightAction?: Snippet;
	} = $props();
</script>

<header class="topbar" role="banner">
	<button
		class="topbar-btn"
		aria-label={canGoBack ? 'Go back' : 'Open navigation'}
		onclick={canGoBack ? onBack : onMenuToggle}
	>
		{#if canGoBack}
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="24" height="24">
				<path d="M15 18l-6-6 6-6"/>
			</svg>
		{:else}
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="24" height="24">
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
		{:else}
			<div class="topbar-spacer"></div>
		{/if}
	</div>
</header>

<style>
	.topbar {
		display: flex;
		align-items: center;
		height: 48px;
		padding: 0 var(--s-2);
		padding-top: env(safe-area-inset-top);
		background: var(--paper);
		border-bottom: 1px solid var(--rule);
		flex-shrink: 0;
		z-index: 100;
		gap: var(--s-2);
	}

	.topbar-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 44px;
		height: 44px;
		border: none;
		background: none;
		color: var(--ink);
		cursor: pointer;
		border-radius: var(--r-sm);
		flex-shrink: 0;
	}

	.topbar-btn:hover {
		background: var(--paper-3);
	}

	.topbar-title {
		flex: 1;
		font-family: var(--font-display);
		font-size: 18px;
		font-weight: 600;
		letter-spacing: -0.4px;
		color: var(--ink);
		text-align: center;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.topbar-right {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		width: 44px;
		height: 44px;
		flex-shrink: 0;
	}

	.topbar-spacer {
		width: 44px;
	}
</style>
