<script lang="ts">
	let {
		name,
		size,
		updatedAt,
		onClick,
	}: {
		name: string;
		size?: number;
		updatedAt?: string;
		onClick: () => void;
	} = $props();

	function fmtSize(n: number) {
		if (n < 1024) return `${n}B`;
		if (n < 1048576) return `${(n / 1024).toFixed(1)}KB`;
		return `${(n / 1048576).toFixed(1)}MB`;
	}

	function fmtDate(ts: string) {
		return new Date(ts).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
	}
</script>

<button class="artifact-row" onclick={onClick}>
	<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
		stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
		width="28" height="28" class="file-icon" aria-hidden="true">
		<path d="M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9z"/>
		<polyline points="13 2 13 9 20 9"/>
	</svg>
	<div class="artifact-info">
		<span class="artifact-name">{name}</span>
		<span class="artifact-meta">
			{#if size != null}{fmtSize(size)}{/if}
			{#if updatedAt} · {fmtDate(updatedAt)}{/if}
		</span>
	</div>
	<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
		stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
		width="16" height="16" class="row-arrow" aria-hidden="true">
		<path d="M9 18l6-6-6-6"/>
	</svg>
</button>

<style>
	.artifact-row {
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
		min-height: var(--hit, 44px);
		transition: background var(--duration-fast) var(--ease-standard);  /* [feedback] */
	}
	.artifact-row:hover { background: var(--color-bg-hover); }
	.artifact-row:focus-visible {
		outline:        var(--focus-ring);
		outline-offset: var(--focus-ring-offset);
	}

	.file-icon { color: var(--color-fg-subtle); flex-shrink: 0; }

	.artifact-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow: hidden;
		min-width: 0;
	}

	.artifact-name {
		font-family: var(--font-family-sans);
		font-size: var(--font-size-body);
		color: var(--color-fg);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.artifact-meta {
		font-family: var(--font-mono);
		font-size: var(--font-size-label);
		color: var(--color-fg-subtle);
	}

	.row-arrow { color: var(--color-fg-subtle); flex-shrink: 0; }

	@media (prefers-reduced-motion: reduce) {
		.artifact-row { transition: none; }
	}
</style>
