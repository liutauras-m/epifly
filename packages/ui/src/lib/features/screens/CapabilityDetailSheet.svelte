<svelte:options runes={true} />
<script lang="ts">
  import Sheet from '../../components/Sheet.svelte';
  import type { CapEntry } from '../CapabilityBrowser.svelte';

  let {
    open,
    capability,
    onclose,
    onInvoke,
  }: {
    open: boolean;
    capability: CapEntry | null;
    onclose: () => void;
    /** Called with the full capability so the parent can build a rich invocation prompt. */
    onInvoke: (cap: CapEntry) => void;
  } = $props();
</script>

<Sheet {open} {onclose} label={capability?.name ?? 'Capability detail'}>
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

        <button class="invoke-btn" onclick={() => { onInvoke(capability); onclose(); }}>
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
</Sheet>

<style>
	.detail-body {
		padding: var(--space-4);
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	.detail-kind { display: flex; }

	.kind-badge {
		font-family: var(--font-family-mono);
		font-size: var(--font-size-label);
		background: var(--color-accent-soft);
		color: var(--color-accent-hover);
		padding: 4px var(--space-2);
		border-radius: var(--radius-sm);
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.detail-desc {
		font-family: var(--font-family-sans);
		font-size: var(--font-size-body);
		color: var(--color-fg-muted);
		line-height: 1.5;
		margin: 0;
	}

	.tools-section { display: flex; flex-direction: column; gap: var(--space-2); }

	.tools-label {
		font-family: var(--font-family-mono);
		font-size: var(--font-size-label);
		letter-spacing: 0.08em;
		color: var(--color-fg-subtle);
		text-transform: uppercase;
	}

	.tool-row {
		padding: var(--space-2) var(--space-3);
		background: var(--color-bg-raised);
		border-radius: var(--radius-sm);
	}

	.tool-name {
		font-family: var(--font-family-mono);
		font-size: var(--font-size-meta);
		color: var(--color-fg);
		display: block;
	}

	.tool-desc {
		font-family: var(--font-family-sans);
		font-size: var(--font-size-meta);
		color: var(--color-fg-subtle);
		display: block;
		margin-top: 2px;
	}

	.invoke-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		height: 48px;
		background: var(--color-accent);
		color: var(--color-fg);
		border: none;
		border-radius: var(--radius-md);
		font-family: var(--font-family-sans);
		font-size: var(--font-size-body);
		font-weight: 600;
		cursor: pointer;
		transition: background var(--duration-fast);  /* [feedback] */
	}
	.invoke-btn:hover { background: var(--color-accent-hover); }
	.invoke-btn:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }

	@media (prefers-reduced-motion: reduce) {
		.invoke-btn { transition: none; }
	}
</style>
