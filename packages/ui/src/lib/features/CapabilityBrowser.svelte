<script lang="ts">
	import type { ConusSdk } from '@conusai/sdk';
	import CapabilityRow from './CapabilityRow.svelte';

	export type CapEntry = {
		name: string;
		description?: string;
		kind?: string;
		tools?: Array<{ name: string; description?: string }>;
	};

	let {
		sdk,
		onSelect,
		showChevron = true,
	}: {
		sdk: ConusSdk;
		/** Called when a capability row is clicked. */
		onSelect: (cap: CapEntry) => void;
		/** Pass false to hide the chevron on each row (desktop panel style). */
		showChevron?: boolean;
	} = $props();

	let query = $state('');
	let caps = $state<CapEntry[]>([]);
	let loading = $state(true);
	let debounceTimer: ReturnType<typeof setTimeout> | undefined;

	$effect(() => {
		loadCaps();
	});

	async function loadCaps() {
		loading = true;
		try {
			if (query.trim()) {
				const res = await sdk.capabilities.search(query);
				caps = (res.data as any)?.results ?? [];
			} else {
				const res = await sdk.capabilities.list();
				const d = res.data as any;
				caps = Array.isArray(d) ? d : (d?.capabilities ?? []);
			}
		} catch {
			caps = [];
		} finally {
			loading = false;
		}
	}

	function onQueryInput(e: Event) {
		query = (e.target as HTMLInputElement).value;
		clearTimeout(debounceTimer);
		debounceTimer = setTimeout(loadCaps, 220);
	}
</script>

<div class="cap-browser">
	<div class="search-bar">
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75"
			stroke-linecap="round" stroke-linejoin="round" width="18" height="18"
			class="search-icon" aria-hidden="true">
			<circle cx="11" cy="11" r="8"/>
			<line x1="21" y1="21" x2="16.65" y2="16.65"/>
		</svg>
		<input
			class="search-input"
			type="search"
			placeholder="Search capabilities…"
			value={query}
			oninput={onQueryInput}
			aria-label="Search capabilities"
		/>
	</div>

	{#if loading}
		<div class="loading-list" role="list" aria-label="Loading capabilities">
			{#each [1, 2, 3, 4, 5] as _}
				<div class="skeleton-cap" role="presentation"></div>
			{/each}
		</div>
	{:else if caps.length === 0}
		<div class="empty">
			<p>No capabilities found{query.trim() ? ` for "${query}"` : ''}.</p>
		</div>
	{:else}
		<div class="caps-list" role="list">
			{#each caps as cap (cap.name)}
				<CapabilityRow
					name={cap.name}
					description={cap.description}
					kind={cap.kind ?? 'tool'}
					toolCount={Array.isArray(cap.tools) ? cap.tools.length : 0}
					{showChevron}
					onClick={() => onSelect(cap)}
				/>
			{/each}
		</div>
	{/if}
</div>

<style>
	.cap-browser {
		display: flex;
		flex-direction: column;
		flex: 1;
		overflow: hidden;
	}

	.search-bar {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-4);
		border-bottom: 1px solid var(--color-border);
		flex-shrink: 0;
	}

	.search-icon { color: var(--color-fg-subtle); flex-shrink: 0; }

	.search-input {
		flex: 1;
		height: 40px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-md);
		padding: 0 var(--space-3);
		background: var(--color-bg-raised);
		color: var(--color-fg);
		font-family: var(--font-family-sans);
		font-size: var(--font-size-body);
		/* Prevent iOS zoom on focus */
		font-size: max(16px, var(--font-size-body));
	}

	.search-input:focus {
		outline: none;
		border-color: var(--color-accent);
		box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-accent) 20%, transparent);
	}

	.caps-list {
		flex: 1;
		overflow-y: auto;
	}

	.loading-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		padding: var(--space-4);
	}

	.skeleton-cap {
		height: 64px;
		background: var(--color-bg-raised);
		border-radius: var(--radius-sm);
		animation: shimmer 1.2s ease-in-out infinite;  /* [feedback] */
	}

	@keyframes shimmer {
		0%, 100% { opacity: 1; }
		50%       { opacity: 0.5; }
	}

	@media (prefers-reduced-motion: reduce) {
		.skeleton-cap { animation: none; }
	}

	.empty {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-fg-subtle);
		font-family: var(--font-family-sans);
		font-size: var(--font-size-body);
		padding: var(--space-8);
	}
	.empty p { margin: 0; }
</style>
