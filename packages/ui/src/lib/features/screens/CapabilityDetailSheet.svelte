<script lang="ts">
	import AppBottomSheet from '../chrome/AppBottomSheet.svelte';
	import type { CapEntry } from '../CapabilityBrowser.svelte';

	let {
		open,
		capability,
		onClose,
		onInvoke,
	}: {
		open: boolean;
		capability: CapEntry | null;
		onClose: () => void;
		/** Called with the full capability so the parent can build a rich invocation prompt. */
		onInvoke: (cap: CapEntry) => void;
	} = $props();
</script>

<AppBottomSheet {open} {onClose} title={capability?.name ?? ''}>
	{#snippet children()}
		{#if capability}
			<div class="detail-body">
				<div class="detail-kind">
					<span class="kind-badge">{capability.kind ?? 'tool'}</span>
				</div>

				{#if capability.description}
					<p class="detail-desc">{capability.description}</p>
				{/if}

				{#if capability.tools?.length}
					<div class="tools-section">
						<div class="tools-label">Tools</div>
						{#each capability.tools as tool}
							<div class="tool-row">
								<span class="tool-name">{tool.name}</span>
								{#if tool.description}
									<span class="tool-desc">{tool.description}</span>
								{/if}
							</div>
						{/each}
					</div>
				{/if}

				<button class="invoke-btn" onclick={() => { onInvoke(capability); onClose(); }}>
					Invoke in current workspace
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
						stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"
						width="18" height="18" aria-hidden="true">
						<path d="M4 10h12M10 4l6 6-6 6"/>
					</svg>
				</button>
			</div>
		{/if}
	{/snippet}
</AppBottomSheet>

<style>
	.detail-body {
		padding: var(--s-4);
		display: flex;
		flex-direction: column;
		gap: var(--s-4);
	}

	.detail-kind { display: flex; }

	.kind-badge {
		font-family: var(--font-mono);
		font-size: var(--t-label);
		background: var(--ember-soft);
		color: var(--ember-2);
		padding: 4px var(--s-2);
		border-radius: var(--r-sm);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.detail-desc {
		font-family: var(--font-body);
		font-size: var(--t-body);
		color: var(--ink-2);
		line-height: 1.5;
		margin: 0;
	}

	.tools-section { display: flex; flex-direction: column; gap: var(--s-2); }

	.tools-label {
		font-family: var(--font-mono);
		font-size: var(--t-label);
		letter-spacing: 0.08em;
		color: var(--ink-3);
		text-transform: uppercase;
	}

	.tool-row {
		padding: var(--s-2) var(--s-3);
		background: var(--paper-2);
		border-radius: var(--r-sm);
	}

	.tool-name {
		font-family: var(--font-mono);
		font-size: var(--t-meta);
		color: var(--ink);
		display: block;
	}

	.tool-desc {
		font-family: var(--font-body);
		font-size: var(--t-meta);
		color: var(--ink-3);
		display: block;
		margin-top: 2px;
	}

	.invoke-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--s-2);
		height: 48px;
		background: var(--ember);
		color: var(--ink);
		border: none;
		border-radius: var(--r-md);
		font-family: var(--font-body);
		font-size: var(--t-body);
		font-weight: 600;
		cursor: pointer;
		transition: background var(--dur-1);
	}
	.invoke-btn:hover { background: var(--ember-2); }
	.invoke-btn:focus-visible { outline: 2px solid var(--ember); outline-offset: 2px; }

	@media (prefers-reduced-motion: reduce) {
		.invoke-btn { transition: none; }
	}
</style>
