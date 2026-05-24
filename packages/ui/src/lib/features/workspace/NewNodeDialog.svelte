<script lang="ts">
	let { parentName, onsubmit, oncancel }: {
		parentName?: string;
		onsubmit: (kind: 'folder' | 'conversation', name: string) => void;
		oncancel: () => void;
	} = $props();

	let kind = $state<'folder' | 'conversation'>('folder');
	let name = $state('');
	let error = $state('');

	function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		const trimmed = kind === 'conversation' && !name.trim().endsWith('.md')
			? name.trim() + '.md'
			: name.trim();
		if (!trimmed) { error = 'Name is required'; return; }
		onsubmit(kind, trimmed);
	}
</script>

<div class="dialog-backdrop" role="presentation" onclick={oncancel} onkeydown={() => {}}>
	<div class="dialog" role="dialog" aria-modal="true" aria-labelledby="new-node-title"
		onclick={(e) => e.stopPropagation()}>
		<h2 id="new-node-title" class="dialog-title">New item{parentName ? ` in ${parentName}` : ''}</h2>

		<fieldset class="kind-group">
			<legend class="kind-legend">Type</legend>
			<label class="kind-option">
				<input type="radio" name="kind" value="folder" checked={kind === 'folder'}
					onchange={() => kind = 'folder'} /> Folder
			</label>
			<label class="kind-option">
				<input type="radio" name="kind" value="conversation" checked={kind === 'conversation'}
					onchange={() => kind = 'conversation'} /> Conversation (.md)
			</label>
		</fieldset>

		<form onsubmit={handleSubmit}>
			<label class="dialog-field">
				<span class="label-text">Name</span>
				<!-- svelte-ignore a11y_autofocus -->
				<input class="dialog-input" type="text" bind:value={name} required maxlength={255}
					autofocus autocomplete="off"
					placeholder={kind === 'conversation' ? 'New conversation.md' : 'My folder'} />
			</label>
			{#if error}<p class="dialog-error">{error}</p>{/if}
			<div class="dialog-actions">
				<button type="button" class="btn-ghost" onclick={oncancel}>Cancel</button>
				<button type="submit" class="btn-primary">Create</button>
			</div>
		</form>
	</div>
</div>

<style>
	.dialog-backdrop { position: fixed; inset: 0; background: var(--color-backdrop); display: flex; align-items: center; justify-content: center; z-index: 1000; }
	.dialog { background: var(--color-bg-raised); border-radius: var(--radius-md); padding: var(--space-5); max-width: 24rem; width: 90%; box-shadow: 0 8px 32px var(--color-shadow-md); }
	.dialog-title { font-size: var(--font-size-body); font-weight: 600; margin: 0 0 var(--space-3); }
	.kind-group { border: none; padding: 0; margin: 0 0 var(--space-3); display: flex; gap: var(--space-4); }
	.kind-legend { font-size: var(--font-size-meta); opacity: 0.65; margin-bottom: var(--space-1); float: left; width: 100%; }
	.kind-option { font-size: var(--font-size-meta); display: flex; align-items: center; gap: var(--space-1); cursor: pointer; }
	.dialog-field { display: flex; flex-direction: column; gap: var(--space-1); margin-bottom: var(--space-3); }
	.label-text { font-size: var(--font-size-meta); opacity: 0.65; }
	.dialog-input { border: 1px solid var(--color-border); border-radius: var(--radius-xs); padding: var(--space-2) var(--space-2); font-size: var(--font-size-meta); width: 100%; }
	.dialog-input:focus-visible { outline: var(--focus-ring); outline-offset: var(--focus-ring-offset); border-color: var(--color-accent); }
	.dialog-error { color: var(--color-danger); font-size: var(--font-size-meta); margin: 0 0 var(--space-2); }
	.dialog-actions { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-4); }
	.btn-ghost { background: none; border: 1px solid var(--color-border); border-radius: var(--radius-xs); padding: var(--space-1) var(--space-3); cursor: pointer; font-size: var(--font-size-meta); }
	.btn-primary { background: var(--color-accent); color: var(--color-on-accent); border: none; border-radius: var(--radius-xs); padding: var(--space-1) var(--space-3); cursor: pointer; font-size: var(--font-size-meta); }
	.btn-primary:hover { background: var(--color-accent-hover); }
</style>
