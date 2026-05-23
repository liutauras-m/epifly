<script lang="ts">
	import { AppBottomSheet } from '@conusai/ui/features';
	import { tap, startViewTransition } from '@conusai/ui/motion';

	let {
		open,
		name,
		plan,
		onClose,
		onLogout,
	}: {
		open: boolean;
		name: string;
		plan: string;
		onClose: () => void;
		onLogout: () => void;
	} = $props();

	let currentTheme = $state(
		typeof document !== 'undefined'
			? (document.documentElement.dataset.theme ?? 'paper')
			: 'paper'
	);

	function toggleTheme() {
		startViewTransition(() => {
			currentTheme = currentTheme === 'paper' ? 'forge' : 'paper';
			document.documentElement.dataset.theme = currentTheme;
			localStorage.setItem('conusai-theme', currentTheme);
		});
	}

	function initials(n: string) {
		return n.split(' ').map(w => w[0]).join('').slice(0, 2).toUpperCase();
	}

	const APP_VERSION = '0.4.0';
</script>

<AppBottomSheet {open} {onClose} title="Profile">
	{#snippet children()}
		<div class="profile-content">
			<div class="profile-avatar-row">
				<div class="big-avatar">{initials(name)}</div>
				<div>
					<div class="profile-name">{name}</div>
					<div class="profile-plan">{plan}</div>
				</div>
			</div>

			<div class="divider"></div>

			<button class="action-row" use:tap onclick={toggleTheme}>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="20" height="20">
					{#if currentTheme === 'paper'}
						<path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/>
					{:else}
						<circle cx="12" cy="12" r="5"/>
						<line x1="12" y1="1" x2="12" y2="3"/>
						<line x1="12" y1="21" x2="12" y2="23"/>
						<line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/>
						<line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/>
						<line x1="1" y1="12" x2="3" y2="12"/>
						<line x1="21" y1="12" x2="23" y2="12"/>
						<line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/>
						<line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/>
					{/if}
				</svg>
				<span>{currentTheme === 'paper' ? 'Switch to dark theme' : 'Switch to light theme'}</span>
			</button>

			<div class="divider"></div>

			<!-- Billing & Usage (opens web app account page) -->
			<a
				class="action-row"
				href="/account/billing"
				target="_blank"
				rel="noopener noreferrer"
				use:tap
				onclick={onClose}
			>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="20" height="20">
					<rect x="1" y="4" width="22" height="16" rx="2" ry="2"/>
					<line x1="1" y1="10" x2="23" y2="10"/>
				</svg>
				<span>Billing &amp; Usage</span>
			</a>

			<div class="divider"></div>

			<button class="action-row danger" use:tap onclick={() => { onLogout(); onClose(); }}>
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" width="20" height="20">
					<path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4"/>
					<polyline points="16 17 21 12 16 7"/>
					<line x1="21" y1="12" x2="9" y2="12"/>
				</svg>
				<span>Sign out</span>
			</button>

			<div class="version">v{APP_VERSION} · ConusAI</div>
		</div>
	{/snippet}
</AppBottomSheet>

<style>
	.profile-content {
		display: flex;
		flex-direction: column;
	}

	.profile-avatar-row {
		display: flex;
		align-items: center;
		gap: var(--s-4);
		padding: var(--s-4);
	}

	.big-avatar {
		width: 56px;
		height: 56px;
		border-radius: var(--r-full);
		background: var(--paper-3);
		font-family: var(--font-display);
		font-size: 20px;
		font-weight: 600;
		color: var(--ink);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.profile-name {
		font-family: var(--font-body);
		font-size: 17px;
		font-weight: 600;
		color: var(--ink);
	}

	.profile-plan {
		font-family: var(--font-mono);
		font-size: 12px;
		color: var(--ink-3);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		margin-top: 2px;
	}

	.divider {
		height: 1px;
		background: var(--rule);
		margin: 0 var(--s-4);
	}

	.action-row {
		display: flex;
		align-items: center;
		gap: var(--s-3);
		height: 56px;
		padding: 0 var(--s-4);
		border: none;
		background: none;
		font-family: var(--font-body);
		font-size: 16px;
		color: var(--ink);
		cursor: pointer;
		width: 100%;
		text-align: left;
		text-decoration: none;
		transition: background var(--dur-1);
	}

	.action-row:hover { background: var(--paper-2); }

	@media (prefers-reduced-motion: reduce) {
		.action-row { transition: none; }
	}

	.action-row.danger { color: var(--danger); }

	.version {
		padding: var(--s-4);
		font-family: var(--font-mono);
		font-size: 11px;
		color: var(--ink-3);
		text-align: center;
	}
</style>
