<script lang="ts">
	import { tap } from '../motion/index.js';

	let {
		suggestions,
		onSelect,
		scrollable = false,
		label = 'Suggested prompts',
		baseDelay = 0,
	}: {
		suggestions: string[];
		onSelect: (text: string) => void;
		/** Renders as a single horizontally-scrollable row (mobile). Default: wrapping grid. */
		scrollable?: boolean;
		/** Accessible label for the list. */
		label?: string;
		/**
		 * Base animation-delay in ms before the first chip appears.
		 * Phase 6 page-load cascade: pass 680 from ChatScreen so chips animate
		 * at 680–920 ms — after the composer (560 ms) has settled.
		 */
		baseDelay?: number;
	} = $props();
</script>

<ul class="chips" class:scrollable aria-label={label}>
	{#each suggestions.slice(0, 4) as s, i (s)}
		<li class="chip-item">
			<button
				class="chip"
				use:tap
				style="animation-delay: {baseDelay + i * 40}ms"
				onclick={() => onSelect(s)}
			>
				{s}
			</button>
		</li>
	{/each}
</ul>

<style>
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		justify-content: center;
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.chips.scrollable {
		flex-wrap: nowrap;
		overflow-x: auto;
		justify-content: flex-start;
		padding: var(--space-2) var(--space-4);
		scrollbar-width: none;
		-webkit-overflow-scrolling: touch;
	}
	.chips.scrollable::-webkit-scrollbar { display: none; }

	.chip-item {
		display: contents;
	}

	.chip {
		background: var(--color-bg);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-full);
		padding: 0 var(--space-3);
		min-height: var(--chip-h-md, 32px);
		display: inline-flex;
		align-items: center;
		justify-content: center;
		font-family: var(--font-family-sans);
		font-size: var(--font-size-label, 13px);
		color: var(--color-fg-muted);
		cursor: pointer;
		opacity: 0;
		animation: chip-in 220ms var(--ease-out, cubic-bezier(0.22, 1, 0.36, 1)) forwards;  /* [hierarchy] */
		white-space: nowrap;
		transition: border-color var(--duration-fast), color var(--duration-fast), background var(--duration-fast); /* [feedback] */
	}

	.chip:hover {
		border-color: var(--color-accent-border, var(--color-border));
		background: var(--color-bg-hover);
		color: var(--color-fg);
	}

	.chip:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 2px;
	}

	@keyframes chip-in {
		from { opacity: 0; transform: translateY(8px); }
		to   { opacity: 1; transform: translateY(0); }
	}

	@media (prefers-reduced-motion: reduce) {
		.chip { animation: none; opacity: 1; transition: none; }
	}
</style>
