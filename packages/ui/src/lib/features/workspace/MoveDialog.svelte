<script lang="ts">
	let { nodeName, onmove, oncancel }: {
		nodeName: string;
		onmove: (newParentPath: string | null) => void;
		oncancel: () => void;
	} = $props();

	let dest = $state('');
</script>

<div class="dialog-backdrop" role="presentation" onclick={oncancel} onkeydown={() => {}}>
	<div class="dialog" role="dialog" aria-modal="true" aria-labelledby="move-title"
		onclick={(e) => e.stopPropagation()}>
		<h2 id="move-title" class="dialog-title">Move "{nodeName}"</h2>
		<label class="dialog-field">
			<span class="label-text">New parent folder path (empty = root)</span>
			<input class="dialog-input" type="text" placeholder="e.g. Projects/2026"
				bind:value={dest} autocomplete="off" />
		</label>
		<div class="dialog-actions">
			<button class="btn-ghost" onclick={oncancel}>Cancel</button>
			<button class="btn-primary" onclick={() => onmove(dest.trim() || null)}>Move</button>
		</div>
	</div>
</div>

<style>
	.dialog-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.3); display: flex; align-items: center; justify-content: center; z-index: 1000; }
	.dialog { background: var(--color-bg-raised); border-radius: var(--radius-md); padding: var(--space-5); max-width: 24rem; width: 90%; box-shadow: 0 8px 32px var(--color-shadow-md); }
	.dialog-title { font-size: 1rem; font-weight: 600; margin: 0 0 1rem; }
	.dialog-field { display: flex; flex-direction: column; gap: 0.375rem; margin-bottom: 1.25rem; }
	.label-text { font-size: 0.8125rem; opacity: 0.65; }
	.dialog-input { border: 1px solid var(--border, #d1cdc8); border-radius: 0.25rem; padding: 0.5rem 0.625rem; font-size: 0.875rem; width: 100%; }
	.dialog-actions { display: flex; justify-content: flex-end; gap: 0.5rem; }
	.btn-ghost { background: none; border: 1px solid var(--border, #d1cdc8); border-radius: 0.25rem; padding: 0.375rem 0.875rem; cursor: pointer; font-size: 0.875rem; }
	.btn-primary { background: var(--color-accent); color: var(--color-on-accent); border: none; border-radius: var(--radius-xs); padding: var(--space-1) var(--space-3); cursor: pointer; font-size: var(--font-size-meta); }
</style>
