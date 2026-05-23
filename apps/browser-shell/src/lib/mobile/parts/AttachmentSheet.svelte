<script lang="ts">
	import MobileBottomSheet from '../chrome/MobileBottomSheet.svelte';
	import type { Attachment } from '@conusai/ui/features';

	let {
		open,
		onClose,
		onAdd,
		onUpload,
	}: {
		open: boolean;
		onClose: () => void;
		onAdd: (atts: Attachment[]) => void;
		onUpload: (files: File[]) => Promise<Attachment[]>;
	} = $props();

	let fileInput: HTMLInputElement;

	async function handleFiles(files: FileList | null) {
		if (!files || files.length === 0) return;
		const result = await onUpload(Array.from(files));
		if (result.length) { onAdd(result); onClose(); }
	}
</script>

<MobileBottomSheet {open} {onClose} title="Add attachment">
	{#snippet children()}
		<div class="att-list">
			<button class="att-row" onclick={() => fileInput?.click()}>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="22" height="22">
					<path d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"/>
				</svg>
				<span>Choose file</span>
			</button>
		</div>
		<input
			bind:this={fileInput}
			type="file"
			multiple
			style="display:none"
			onchange={(e) => handleFiles((e.target as HTMLInputElement).files)}
		/>
	{/snippet}
</MobileBottomSheet>

<style>
	.att-list { display: flex; flex-direction: column; }

	.att-row {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		height: 56px;
		padding: 0 var(--space-5);
		border: none;
		background: none;
		font-family: var(--font-family-sans);
		font-size: 16px;
		color: var(--ink);
		cursor: pointer;
		width: 100%;
		text-align: left;
		border-bottom: 1px solid var(--rule);
		transition: background var(--duration-fast);
	}

	.att-row:hover { background: var(--paper-2); }

	@media (prefers-reduced-motion: reduce) {
		.att-row { transition: none; }
	}
</style>
