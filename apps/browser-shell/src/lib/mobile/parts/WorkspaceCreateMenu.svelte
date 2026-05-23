<script lang="ts">
	let {
		onNewFolder,
		onNewConversation,
		onClose,
	}: {
		onNewFolder: () => void;
		onNewConversation: () => void;
		onClose: () => void;
	} = $props();
</script>

<div class="menu-backdrop" aria-hidden="true" onclick={onClose}></div>
<div class="menu" role="menu">
	<button class="menu-item" role="menuitem" onclick={() => { onNewFolder(); onClose(); }}>
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="20" height="20">
			<path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"/>
			<line x1="12" y1="11" x2="12" y2="17"/>
			<line x1="9" y1="14" x2="15" y2="14"/>
		</svg>
		New folder
	</button>
	<button class="menu-item" role="menuitem" onclick={() => { onNewConversation(); onClose(); }}>
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="20" height="20">
			<path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/>
			<line x1="12" y1="9" x2="12" y2="13"/>
			<line x1="10" y1="11" x2="14" y2="11"/>
		</svg>
		New chat
	</button>
</div>

<style>
	.menu-backdrop {
		position: fixed;
		inset: 0;
		z-index: 200;
	}

	.menu {
		position: absolute;
		top: 100%;
		right: 0;
		background: var(--paper);
		border: 1px solid var(--rule);
		border-radius: var(--radius-md);
		box-shadow: 0 8px 24px var(--shadow-md);
		z-index: 210;
		min-width: 180px;
		overflow: hidden;
	}

	.menu-item {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		width: 100%;
		height: 48px;
		padding: 0 var(--space-4);
		border: none;
		background: none;
		font-family: var(--font-family-sans);
		font-size: 15px;
		color: var(--ink);
		cursor: pointer;
		text-align: left;
	}

	.menu-item:hover { background: var(--paper-2); }
	.menu-item + .menu-item { border-top: 1px solid var(--rule); }
</style>
