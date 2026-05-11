<script lang="ts">
	import type { ConusSdk } from '@conusai/sdk';
	import CapabilityRow from '../parts/CapabilityRow.svelte';
	import CapabilityDetailSheet from '../parts/CapabilityDetailSheet.svelte';

	let {
		sdk,
		onInvoke,
	}: {
		sdk: ConusSdk;
		onInvoke: (name: string) => void;
	} = $props();

	let query = $state('');
	let caps = $state<any[]>([]);
	let loading = $state(true);
	let selectedCap = $state<any | null>(null);
	let sheetOpen = $state(false);
	let debounceTimer: ReturnType<typeof setTimeout>;

	$effect(() => {
		loadCaps();
	});

	async function loadCaps() {
		loading = true;
		if (query.trim()) {
			const res = await sdk.capabilities.search(query);
			caps = (res.data as any)?.results ?? [];
		} else {
			const res = await sdk.capabilities.list();
			const d = res.data as any;
			caps = Array.isArray(d) ? d : (d?.capabilities ?? []);
		}
		loading = false;
	}

	function onQueryInput(e: Event) {
		query = (e.target as HTMLInputElement).value;
		clearTimeout(debounceTimer);
		debounceTimer = setTimeout(loadCaps, 200);
	}

	function openDetail(cap: any) {
		selectedCap = cap;
		sheetOpen = true;
	}
</script>

<div class="caps-screen">
	<div class="search-bar">
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="18" height="18" class="search-icon">
			<circle cx="11" cy="11" r="8"/>
			<line x1="21" y1="21" x2="16.65" y2="16.65"/>
		</svg>
		<input
			class="search-input"
			type="search"
			placeholder="Search capabilities..."
			value={query}
			oninput={onQueryInput}
			aria-label="Search capabilities"
		/>
	</div>

	{#if loading}
		<div class="loading-list">
			{#each [1,2,3,4,5] as _}
				<div class="skeleton-cap"></div>
			{/each}
		</div>
	{:else if caps.length === 0}
		<div class="empty">
			<p>No matching capabilities.</p>
		</div>
	{:else}
		<div class="caps-list" role="list">
			{#each caps as cap (cap.name)}
				<CapabilityRow
					name={cap.name}
					description={cap.description}
					kind={cap.kind}
					onClick={() => openDetail(cap)}
				/>
			{/each}
		</div>
	{/if}
</div>

<CapabilityDetailSheet
	open={sheetOpen}
	capability={selectedCap}
	onClose={() => sheetOpen = false}
	onInvoke={(name) => { onInvoke(name); sheetOpen = false; }}
/>

<style>
	.caps-screen {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background: var(--paper);
	}

	.search-bar {
		display: flex;
		align-items: center;
		gap: var(--s-2);
		padding: var(--s-3) var(--s-4);
		border-bottom: 1px solid var(--rule);
		flex-shrink: 0;
	}

	.search-icon { color: var(--ink-3); flex-shrink: 0; }

	.search-input {
		flex: 1;
		height: 40px;
		border: 1px solid var(--rule);
		border-radius: var(--r-md);
		padding: 0 var(--s-3);
		background: var(--paper-2);
		color: var(--ink);
		font-family: var(--font-body);
		font-size: 15px;
	}

	.search-input:focus {
		outline: none;
		border-color: var(--ember);
	}

	.caps-list {
		flex: 1;
		overflow-y: auto;
	}

	.loading-list {
		display: flex;
		flex-direction: column;
		gap: var(--s-2);
		padding: var(--s-4);
	}

	.skeleton-cap {
		height: 64px;
		background: var(--paper-2);
		border-radius: var(--r-sm);
		animation: shimmer 1.2s ease-in-out infinite;
	}

	@keyframes shimmer {
		0%, 100% { opacity: 1; }
		50% { opacity: 0.5; }
	}

	@media (prefers-reduced-motion: reduce) {
		.skeleton-cap { animation: none; }
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
