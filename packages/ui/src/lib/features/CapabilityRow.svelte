<script lang="ts">
	let {
		name,
		description = '',
		kind,
		toolCount = 0,
		showChevron = true,
		onClick,
	}: {
		name: string;
		description?: string;
		kind: string;
		/** Number of tools this capability exposes, shown as metadata. */
		toolCount?: number;
		/** Show the trailing chevron arrow. Default true. */
		showChevron?: boolean;
		onClick: () => void;
	} = $props();
</script>

<button class="cap-row" onclick={onClick}>
	<div class="cap-main">
		<div class="cap-header">
			<span class="cap-name">{name}</span>
			<span class="cap-kind">{kind}</span>
		</div>
		{#if description}
			<div class="cap-desc">{description}</div>
		{/if}
	</div>
	{#if toolCount > 0}
		<span class="cap-tools">{toolCount} tool{toolCount !== 1 ? 's' : ''}</span>
	{/if}
	{#if showChevron}
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
			stroke-linecap="round" stroke-linejoin="round" width="16" height="16"
			class="cap-arrow" aria-hidden="true">
			<path d="M9 18l6-6-6-6"/>
		</svg>
	{/if}
</button>

<style>
	.cap-row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border: none;
		background: none;
		cursor: pointer;
		width: 100%;
		text-align: left;
		border-bottom: 1px solid var(--color-border);
		transition: background var(--duration-fast); /* [feedback] */
	}

	.cap-row:hover { background: var(--color-bg-raised); }
	.cap-row:focus-visible { outline: var(--focus-ring); outline-offset: var(--focus-ring-offset); }

	.cap-main { flex: 1; min-width: 0; }

	.cap-header {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex-wrap: wrap;
	}

	.cap-name {
		font-family: var(--font-family-sans);
		font-size: var(--font-size-body);
		font-weight: 600;
		color: var(--color-fg);
	}

	.cap-kind {
		font-family: var(--font-family-mono);
		font-size: var(--font-size-label);
		background: var(--color-accent-soft);
		color: var(--color-accent-hover);
		padding: 2px var(--space-2);
		border-radius: var(--radius-sm);
		flex-shrink: 0;
		text-transform: lowercase;
	}

	.cap-desc {
		font-family: var(--font-family-sans);
		font-size: var(--font-size-meta);
		color: var(--color-fg-muted);
		margin-top: 2px;
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
	}

	.cap-tools {
		font-family: var(--font-family-mono);
		font-size: var(--font-size-label, 11px);
		color: var(--color-fg-subtle);
		flex-shrink: 0;
	}

	.cap-arrow { color: var(--color-fg-subtle); flex-shrink: 0; }

	@media (prefers-reduced-motion: reduce) {
		.cap-row { transition: none; }
	}
</style>
