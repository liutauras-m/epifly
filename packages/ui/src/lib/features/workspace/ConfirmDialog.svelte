<script lang="ts">
	let { message, onconfirm, oncancel }: {
		message: string;
		onconfirm: () => void;
		oncancel: () => void;
	} = $props();
</script>

<div class="dialog-backdrop" role="presentation" onclick={oncancel} onkeydown={() => {}}>
	<div class="dialog" role="alertdialog" aria-modal="true" aria-label="Confirm" onclick={(e) => e.stopPropagation()}>
		<p class="dialog-message">{message}</p>
		<div class="dialog-actions">
			<button class="btn-ghost" onclick={oncancel}>Cancel</button>
			<button class="btn-danger" onclick={onconfirm}>Delete</button>
		</div>
	</div>
</div>

<style>
	.dialog-backdrop {
		position: fixed; inset: 0; background: rgba(0,0,0,0.3);
		display: flex; align-items: center; justify-content: center;
		z-index: 1000;
	}
	.dialog {
		background: var(--surface, #fff); border-radius: 0.5rem;
		padding: 1.5rem; max-width: 24rem; width: 90%;
		box-shadow: 0 8px 32px rgba(0,0,0,0.15);
	}
	.dialog-message { margin: 0 0 1.25rem; font-size: 0.9375rem; }
	.dialog-actions { display: flex; justify-content: flex-end; gap: 0.5rem; }
	.btn-ghost { background: none; border: 1px solid var(--border, #d1cdc8); border-radius: 0.25rem; padding: 0.375rem 0.875rem; cursor: pointer; font-size: 0.875rem; }
	.btn-danger { background: #dc2626; color: #fff; border: none; border-radius: 0.25rem; padding: 0.375rem 0.875rem; cursor: pointer; font-size: 0.875rem; }
	.btn-danger:hover { background: #b91c1c; }
</style>
