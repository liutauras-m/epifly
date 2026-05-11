<script lang="ts">
	import type { ConusSdk } from '@conusai/sdk';
	import ArtifactRow from '../parts/ArtifactRow.svelte';

	let { sdk }: { sdk: ConusSdk } = $props();

	let artifacts = $state<any[]>([]);
	let loading = $state(true);

	$effect(() => { loadArtifacts(); });

	async function loadArtifacts() {
		loading = true;
		const res = await sdk.workspaces.list();
		if (!res.error && res.data) {
			const all: any[] = (res.data as any).nodes ?? [];
			artifacts = all.filter((n: any) => n.kind === 'file');
		}
		loading = false;
	}
</script>

<div class="artifacts-screen">
	{#if loading}
		<div class="loading-list">
			{#each [1,2,3] as _}
				<div class="skeleton-row"></div>
			{/each}
		</div>
	{:else if artifacts.length === 0}
		<div class="empty">
			<p>No artifacts yet.</p>
		</div>
	{:else}
		<div class="artifacts-list">
			{#each artifacts as a (a.id)}
				<ArtifactRow
					name={a.name}
					size={a.size_bytes}
					updatedAt={a.updated_at}
					onClick={() => {}}
				/>
			{/each}
		</div>
	{/if}
</div>

<style>
	.artifacts-screen {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background: var(--paper);
	}

	.artifacts-list { flex: 1; overflow-y: auto; }

	.loading-list {
		display: flex;
		flex-direction: column;
		gap: var(--s-2);
		padding: var(--s-4);
	}

	.skeleton-row {
		height: 60px;
		background: var(--paper-2);
		border-radius: var(--r-sm);
		animation: shimmer 1.2s ease-in-out infinite;
	}

	@keyframes shimmer {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.5; }
	}

	@media (prefers-reduced-motion: reduce) {
		.skeleton-row { animation: none; }
	}

	.empty {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--ink-3);
		font-family: var(--font-body);
		font-size: 15px;
	}
</style>
