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
	.dialog-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.3); display: flex; align-items: center; justify-content: center; z-index: 1000; }
	.dialog { background: var(--surface, #fff); border-radius: 0.5rem; padding: 1.5rem; max-width: 24rem; width: 90%; box-shadow: 0 8px 32px rgba(0,0,0,0.15); }
	.dialog-title { font-size: 1rem; font-weight: 600; margin: 0 0 1rem; }
	.kind-group { border: none; padding: 0; margin: 0 0 1rem; display: flex; gap: 1rem; }
	.kind-legend { font-size: 0.8125rem; opacity: 0.65; margin-bottom: 0.375rem; float: left; width: 100%; }
	.kind-option { font-size: 0.875rem; display: flex; align-items: center; gap: 0.375rem; cursor: pointer; }
	.dialog-field { display: flex; flex-direction: column; gap: 0.375rem; margin-bottom: 1rem; }
	.label-text { font-size: 0.8125rem; opacity: 0.65; }
	.dialog-input { border: 1px solid var(--border, #d1cdc8); border-radius: 0.25rem; padding: 0.5rem 0.625rem; font-size: 0.875rem; width: 100%; }
	.dialog-error { color: #dc2626; font-size: 0.8125rem; margin: 0 0 0.75rem; }
	.dialog-actions { display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 1.25rem; }
	.btn-ghost { background: none; border: 1px solid var(--border, #d1cdc8); border-radius: 0.25rem; padding: 0.375rem 0.875rem; cursor: pointer; font-size: 0.875rem; }
	.btn-primary { background: var(--ember-2, #0d9488); color: #fff; border: none; border-radius: 0.25rem; padding: 0.375rem 0.875rem; cursor: pointer; font-size: 0.875rem; }
</style>
