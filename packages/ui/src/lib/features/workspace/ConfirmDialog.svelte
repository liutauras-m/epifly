<script lang="ts">
	let { message, onconfirm, oncancel }: {
		message: string;
		onconfirm: () => void;
		oncancel: () => void;
	} = $props();

	function handleBackdropKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			oncancel();
		}
	}

	function handleBackdropPointerDown(event: PointerEvent) {
		if (event.target === event.currentTarget) {
			oncancel();
		}
	}
</script>

<div class="dialog-backdrop" role="presentation" tabindex="-1" onpointerdown={handleBackdropPointerDown} onkeydown={handleBackdropKeydown}>
	<div class="dialog" role="alertdialog" tabindex="-1" aria-modal="true" aria-label="Confirm" onpointerdown={(e) => e.stopPropagation()}>
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
		background: var(--color-bg-raised, var(--color-bg-raised)); border-radius: var(--radius-md);
		padding: var(--space-5); max-width: 24rem; width: 90%;
		box-shadow: 0 8px 32px var(--color-shadow-md, rgba(0,0,0,0.15));
	}
	.dialog-message { margin: 0 0 var(--space-4); font-size: var(--font-size-body); }
	.dialog-actions { display: flex; justify-content: flex-end; gap: var(--space-2); }
	.btn-ghost { background: none; border: 1px solid var(--color-border); border-radius: var(--radius-xs); padding: var(--space-1) var(--space-3); cursor: pointer; font-size: var(--font-size-meta); }
	.btn-danger { background: var(--color-danger); color: var(--color-on-danger); border: none; border-radius: var(--radius-xs); padding: var(--space-1) var(--space-3); cursor: pointer; font-size: var(--font-size-meta); }
	.btn-danger:hover { filter: brightness(1.1); }
</style>
