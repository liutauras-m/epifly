<script lang="ts">
	import { tap } from '@conusai/ui/motion';

	let {
		suggestions,
		onSelect,
	}: {
		suggestions: string[];
		onSelect: (text: string) => void;
	} = $props();
</script>

<div class="chips" role="list">
	{#each suggestions.slice(0, 4) as s, i (s)}
		<button
			class="chip"
			role="listitem"
			use:tap
			style="animation-delay: {i * 40}ms"
			onclick={() => onSelect(s)}
		>
			{s}
		</button>
	{/each}
</div>

<style>
	.chips {
		display: flex;
		flex-wrap: nowrap;
		overflow-x: auto;
		gap: var(--s-2);
		padding: var(--s-2) var(--s-4);
		scrollbar-width: none;
		-webkit-overflow-scrolling: touch;
	}
	.chips::-webkit-scrollbar { display: none; }

	.chip {
		background: var(--paper);
		border: 1px solid var(--rule);
		border-radius: var(--r-md);
		padding: var(--s-2) var(--s-3);
		font-family: var(--font-body);
		font-size: 14px;
		color: var(--ink-2);
		cursor: pointer;
		opacity: 0;
		animation: chip-in 220ms var(--ease-out, cubic-bezier(0.22, 1, 0.36, 1)) forwards;
		white-space: nowrap;
		transition: border-color var(--dur-1), color var(--dur-1);
	}

	.chip:hover {
		border-color: var(--ember);
		color: var(--ink);
	}

	@keyframes chip-in {
		from { opacity: 0; transform: translateY(8px); }
		to   { opacity: 1; transform: translateY(0); }
	}

	@media (prefers-reduced-motion: reduce) {
		.chip { animation: none; opacity: 1; }
	}
</style>
