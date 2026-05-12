<script lang="ts">
	let {
		name,
		description = '',
		kind,
		onClick,
	}: {
		name: string;
		description?: string;
		kind: string;
		onClick: () => void;
	} = $props();
</script>

<button class="cap-row" onclick={onClick}>
	<div class="cap-main">
		<div class="cap-name">{name}</div>
		{#if description}
			<div class="cap-desc">{description}</div>
		{/if}
	</div>
	<span class="cap-kind">{kind}</span>
	<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="16" height="16" class="cap-arrow">
		<path d="M9 18l6-6-6-6"/>
	</svg>
</button>

<style>
	.cap-row {
		display: flex;
		align-items: center;
		gap: var(--s-3);
		padding: var(--s-3) var(--s-4);
		border: none;
		background: none;
		cursor: pointer;
		width: 100%;
		text-align: left;
		border-bottom: 1px solid var(--rule);
		transition: background var(--dur-1);
	}

	.cap-row:hover { background: var(--paper-2); }

	@media (prefers-reduced-motion: reduce) {
		.cap-row { transition: none; }
	}

	.cap-main { flex: 1; overflow: hidden; }

	.cap-name {
		font-family: var(--font-body);
		font-size: 15px;
		font-weight: 600;
		color: var(--ink);
	}

	.cap-desc {
		font-family: var(--font-body);
		font-size: 13px;
		color: var(--ink-2);
		margin-top: 2px;
		overflow: hidden;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
	}

	.cap-kind {
		font-family: var(--font-mono);
		font-size: 11px;
		background: var(--ember-soft);
		color: var(--ember-2);
		padding: 2px var(--s-2);
		border-radius: var(--r-sm);
		flex-shrink: 0;
	}

	.cap-arrow { color: var(--ink-3); flex-shrink: 0; }
</style>
