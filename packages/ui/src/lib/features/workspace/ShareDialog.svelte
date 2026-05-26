<script lang="ts">
	import type { ConusSdk } from '@conusai/sdk';
	import type { WorkspaceNode } from '@conusai/types';
	import { toasts } from '../../stores/toast.svelte.js';

	let {
		sdk,
		node,
		onclose,
	}: {
		sdk: ConusSdk;
		node: WorkspaceNode & { shared_with?: string[] };
		onclose: (updated?: WorkspaceNode) => void;
	} = $props();

	let uid = $state('');
	let busy = $state(false);
	let sharedWith = $state<string[]>([]);

	$effect(() => {
		sharedWith = node.shared_with ?? [];
	});

	function handleBackdropKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			onclose();
		}
	}

	function handleBackdropPointerDown(event: PointerEvent) {
		if (event.target === event.currentTarget) {
			onclose();
		}
	}

	async function addShare() {
		const id = uid.trim();
		if (!id) return;
		busy = true;
		const result = await sdk.workspaces.share(node.id, id);
		busy = false;
		if (result.error) { toasts.error(`Share failed: ${result.error.message}`); return; }
		sharedWith = (result.data as WorkspaceNode & { shared_with?: string[] }).shared_with ?? sharedWith;
		uid = '';
	}

	async function removeShare(userId: string) {
		const result = await sdk.workspaces.unshare(node.id, userId);
		if (result.error) { toasts.error(`Unshare failed: ${result.error.message}`); return; }
		sharedWith = (result.data as WorkspaceNode & { shared_with?: string[] }).shared_with ?? sharedWith.filter(u => u !== userId);
	}
</script>

<div class="dialog-backdrop" role="presentation" tabindex="-1" onpointerdown={handleBackdropPointerDown} onkeydown={handleBackdropKeydown}>
	<div class="dialog" role="dialog" tabindex="-1" aria-modal="true" aria-labelledby="share-title"
		onpointerdown={(e) => e.stopPropagation()}>
		<h2 id="share-title" class="dialog-title">Share "{node.name}"</h2>

		{#if sharedWith.length > 0}
			<ul class="shared-list">
				{#each sharedWith as u (u)}
					<li class="shared-row">
						<span>{u}</span>
						<button class="btn-ghost btn-sm" onclick={() => removeShare(u)}>Remove</button>
					</li>
				{/each}
			</ul>
		{:else}
			<p class="empty-hint">Not shared with anyone yet.</p>
		{/if}

		<label class="dialog-field">
			<span class="label-text">Add user ID</span>
			<input class="dialog-input" type="text" placeholder="user-abc123" bind:value={uid}
				autocomplete="off" onkeydown={(e) => e.key === 'Enter' && addShare()} />
		</label>

		<div class="dialog-actions">
			<button class="btn-ghost" onclick={() => onclose()}>Close</button>
			<button class="btn-primary" onclick={addShare} disabled={busy || !uid.trim()}>
				{busy ? '…' : 'Share'}
			</button>
		</div>
	</div>
</div>

<style>
	.dialog-backdrop { position: fixed; inset: 0; background: rgba(0,0,0,0.3); display: flex; align-items: center; justify-content: center; z-index: 1000; }
	.dialog { background: var(--color-bg-raised); border-radius: var(--radius-md); padding: var(--space-5); max-width: 24rem; width: 90%; box-shadow: 0 8px 32px var(--color-shadow-md); }
	.dialog-title { font-size: 1rem; font-weight: 600; margin: 0 0 1rem; }
	.shared-list { list-style: none; padding: 0; margin: 0 0 1rem; display: flex; flex-direction: column; gap: 0.375rem; }
	.shared-row { display: flex; align-items: center; justify-content: space-between; font-size: 0.875rem; }
	.empty-hint { font-size: 0.875rem; opacity: 0.55; margin: 0 0 1rem; }
	.dialog-field { display: flex; flex-direction: column; gap: 0.375rem; margin-bottom: 1.25rem; }
	.label-text { font-size: 0.8125rem; opacity: 0.65; }
	.dialog-input { border: 1px solid var(--color-border, #d1cdc8); border-radius: 0.25rem; padding: 0.5rem 0.625rem; font-size: 0.875rem; width: 100%; }
	.dialog-actions { display: flex; justify-content: flex-end; gap: 0.5rem; }
	.btn-ghost { background: none; border: 1px solid var(--color-border, #d1cdc8); border-radius: 0.25rem; padding: 0.375rem 0.875rem; cursor: pointer; font-size: 0.875rem; }
	.btn-sm { padding: 0.2rem 0.5rem; font-size: 0.8125rem; }
	.btn-primary { background: var(--color-accent); color: var(--color-on-accent); border: none; border-radius: var(--radius-xs); padding: var(--space-1) var(--space-3); cursor: pointer; font-size: var(--font-size-meta); }
	.btn-primary:disabled { opacity: 0.5; cursor: default; }
</style>
