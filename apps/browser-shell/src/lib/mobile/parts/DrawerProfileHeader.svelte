<script lang="ts">
	import { tap } from '@conusai/ui/motion';

	let {
		name,
		plan,
		onOpenProfile,
	}: {
		name: string;
		plan: string;
		onOpenProfile: () => void;
	} = $props();

	function initials(n: string) {
		return n.split(' ').map(w => w[0]).join('').slice(0, 2).toUpperCase();
	}
</script>

<button class="profile-header" use:tap onclick={onOpenProfile} aria-label="Open profile">
	<div class="avatar" aria-hidden="true">{initials(name)}</div>
	<div class="user-info">
		<span class="user-name">{name}</span>
		<span class="user-plan">{plan}</span>
	</div>
	<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="16" height="16" class="chevron">
		<path d="M9 18l6-6-6-6"/>
	</svg>
</button>

<style>
	.profile-header {
		display: flex;
		align-items: center;
		gap: var(--s-3);
		width: 100%;
		padding: var(--s-4);
		border: none;
		background: none;
		cursor: pointer;
		border-bottom: 1px solid var(--rule);
		text-align: left;
		transition: background 120ms;
	}

	.profile-header:hover { background: var(--paper-3); }

	.avatar {
		width: 40px;
		height: 40px;
		border-radius: var(--r-full);
		background: var(--paper-3);
		color: var(--ink);
		font-family: var(--font-display);
		font-size: 15px;
		font-weight: 600;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.user-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
		overflow: hidden;
	}

	.user-name {
		font-family: var(--font-body);
		font-size: 15px;
		font-weight: 500;
		color: var(--ink);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.user-plan {
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--ink-3);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.chevron { color: var(--ink-3); flex-shrink: 0; }
</style>
