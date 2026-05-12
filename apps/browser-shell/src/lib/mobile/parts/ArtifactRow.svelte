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
	<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="32" height="32" class="file-icon">
		<path d="M13 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V9z"/>
		<polyline points="13 2 13 9 20 9"/>
	</svg>
	<div class="artifact-info">
		<span class="artifact-name">{name}</span>
		<span class="artifact-meta">
			{#if size}{fmtSize(size)}{/if}
			{#if updatedAt} · {fmtDate(updatedAt)}{/if}
		</span>
	</div>
	<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="16" height="16" class="row-arrow">
		<path d="M9 18l6-6-6-6"/>
	</svg>
</button>

<style>
	.artifact-row {
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

	.artifact-row:hover { background: var(--paper-2); }

	@media (prefers-reduced-motion: reduce) {
		.artifact-row { transition: none; }
	}

	.file-icon { color: var(--ink-3); flex-shrink: 0; }

	.artifact-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow: hidden;
	}

	.artifact-name {
		font-family: var(--font-body);
		font-size: 15px;
		color: var(--ink);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.artifact-meta {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--ink-3);
	}

	.row-arrow { color: var(--ink-3); flex-shrink: 0; }
</style>
