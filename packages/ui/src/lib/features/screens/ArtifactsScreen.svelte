<script lang="ts">
	import type { ConusSdk } from '@conusai/sdk';
	import ArtifactRow from './ArtifactRow.svelte';

	let { sdk }: { sdk: ConusSdk } = $props();

	type Artifact = {
		id: string;
		name: string;
		kind: string;
		size_bytes?: number;
		updated_at?: string;
	};

	let artifacts = $state<Artifact[]>([]);
	let loading = $state(true);

	$effect(() => { loadArtifacts(); });

	async function loadArtifacts() {
		loading = true;
		// Pull the full workspace tree, then filter to file kinds.
		const res = await sdk.workspaces.tree();
		if (!res.error && res.data) {
			const raw = res.data as any;
			const all: Artifact[] = Array.isArray(raw) ? raw : (raw?.nodes ?? []);
			artifacts = all.filter((n) => n.kind === 'file');
		}
		loading = false;
	}
</script>

<div class="artifacts-screen">
	{#if loading}
		<div class="loading-list" role="list" aria-label="Loading artifacts">
			{#each [1, 2, 3, 4] as _}
				<div class="skeleton-row" role="presentation"></div>
			{/each}
		</div>
	{:else if artifacts.length === 0}
		<div class="empty">
			<p>No artifacts yet — files attached to conversations will appear here.</p>
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
		gap: var(--space-2);
		padding: var(--space-4);
	}

	.skeleton-row {
		height: 60px;
		background: var(--paper-2);
		border-radius: var(--radius-sm);
		animation: shimmer 1.2s ease-in-out infinite;
	}

	@keyframes shimmer {
		0%, 100% { opacity: 1; }
		50%      { opacity: 0.5; }
	}

	.empty {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--space-8);
		color: var(--ink-3);
		font-family: var(--font-family-sans);
		font-size: var(--font-size-body);
		text-align: center;
		max-width: 480px;
		margin: 0 auto;
	}

	.empty p { margin: 0; }

	@media (prefers-reduced-motion: reduce) {
		.skeleton-row { animation: none; }
	}
</style>
