<script lang="ts">
	import { CreditCard, BarChart3, LogOut } from 'lucide-svelte';
	import type { PageData } from './$types';

	export let data: PageData;

	const { user, subscription, authProvider } = data;

	$: planLabel = subscription?.plan_key ?? user?.plan ?? 'free';
	$: statusLabel = subscription?.status ?? 'active';
</script>

<svelte:head>
	<title>Account — ConusAI</title>
</svelte:head>

<div class="account-page">
	<header class="account-header">
		<h1>Account</h1>
	</header>

	<section class="profile-card">
		<div class="avatar" aria-hidden="true">{(user?.name ?? '?')[0].toUpperCase()}</div>
		<div class="profile-info">
			<p class="name">{user?.name ?? 'Unknown'}</p>
			<span class="plan-badge badge-{planLabel.toLowerCase()}">{planLabel.toUpperCase()}</span>
			{#if statusLabel !== 'active' && statusLabel !== 'trialing'}
				<span class="status-badge status-{statusLabel}">{statusLabel.replace('_', ' ')}</span>
			{/if}
		</div>
	</section>

	<nav class="links" aria-label="Account navigation">
		<a href="/account/billing" class="link-card">
			<span class="link-icon" aria-hidden="true">
				<CreditCard size={20} strokeWidth={1.5} />
			</span>
			<div class="link-body">
				<strong>Billing &amp; Plans</strong>
				<p>Manage your subscription, upgrade, or view invoices.</p>
			</div>
		</a>

		<a href="/account/usage" class="link-card">
			<span class="link-icon" aria-hidden="true">
				<BarChart3 size={20} strokeWidth={1.5} />
			</span>
			<div class="link-body">
				<strong>Usage</strong>
				<p>View agent turns, token consumption, and storage.</p>
			</div>
		</a>

		<a
			href={authProvider === 'zitadel' ? '/auth/logout' : '/logout'}
			class="link-card link-destructive"
		>
			<span class="link-icon" aria-hidden="true">
				<LogOut size={20} strokeWidth={1.5} />
			</span>
			<div class="link-body">
				<strong>Sign out</strong>
				<p>End your session.</p>
			</div>
		</a>
	</nav>
</div>

<style>
	.account-page {
		max-width: 640px;
		margin: 0 auto;
		padding: 2rem 1rem;
	}

	.account-header h1 {
		font-family: var(--font-display);
		font-size: 1.75rem;
		font-weight: 800;
		letter-spacing: -0.04em;
		color: var(--ink);
		margin-bottom: 1.5rem;
	}

	/* Profile card */
	.profile-card {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 1.25rem;
		border: 1px solid var(--rule);
		border-radius: var(--r-lg);
		margin-bottom: 1.25rem;
		background: var(--paper);
	}

	.avatar {
		width: 48px;
		height: 48px;
		border-radius: var(--r-full);
		background: var(--ember);
		color: #fff;
		display: flex;
		align-items: center;
		justify-content: center;
		font-family: var(--font-display);
		font-weight: 800;
		font-size: 1.15rem;
		letter-spacing: -0.02em;
		flex-shrink: 0;
	}

	.profile-info {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
	}

	.name {
		font-family: var(--font-body);
		font-weight: 600;
		color: var(--ink);
		margin: 0;
	}

	.plan-badge {
		display: inline-flex;
		align-items: center;
		padding: 0.1rem 0.5rem;
		border-radius: var(--r-full);
		font-family: var(--font-mono);
		font-size: 0.65rem;
		font-weight: 600;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		width: fit-content;
	}

	.badge-free       { background: var(--paper-2); color: var(--ink-3); border: 1px solid var(--rule); }
	.badge-pro        { background: var(--ember-soft); color: var(--ember); border: 1px solid var(--ember-glow); }
	.badge-team       { background: var(--cyan-soft); color: var(--cyan); border: 1px solid rgba(0,212,255,0.28); }
	.badge-enterprise { background: var(--ember-soft); color: var(--ember); border: 1px solid var(--ember-glow); font-weight: 700; }

	.status-badge {
		display: inline-block;
		padding: 0.1rem 0.5rem;
		border-radius: var(--r-full);
		font-family: var(--font-mono);
		font-size: 0.65rem;
		font-weight: 600;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		background: var(--danger-soft);
		color: var(--danger);
		border: 1px solid rgba(179, 36, 0, 0.28);
	}

	/* Nav links */
	.links {
		display: flex;
		flex-direction: column;
		gap: 0.625rem;
	}

	.link-card {
		display: flex;
		align-items: flex-start;
		gap: 1rem;
		padding: 1rem 1.25rem;
		border: 1px solid var(--rule);
		border-radius: var(--r-lg);
		text-decoration: none;
		color: inherit;
		background: var(--paper);
		transition: background 150ms cubic-bezier(0.4, 0, 0.2, 1),
		            box-shadow 150ms cubic-bezier(0.4, 0, 0.2, 1),
		            transform 150ms cubic-bezier(0.4, 0, 0.2, 1);
	}

	.link-card:hover {
		background: var(--paper-2);
		transform: translateY(-1px);
		box-shadow: 0 4px 12px rgba(17, 17, 17, 0.06);
	}

	.link-card:focus-visible {
		outline: none;
		box-shadow: 0 0 0 3px var(--ember-glow);
	}

	.link-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		border-radius: var(--r-sm);
		background: var(--ember-soft);
		color: var(--ember);
		flex-shrink: 0;
		margin-top: 0.1rem;
	}

	.link-body strong {
		display: block;
		font-family: var(--font-body);
		font-weight: 600;
		font-size: 0.9rem;
		color: var(--ink);
		margin-bottom: 0.15rem;
	}

	.link-body p {
		margin: 0;
		font-size: 0.8rem;
		color: var(--ink-3);
		line-height: 1.5;
	}

	.link-destructive {
		border-color: var(--danger-soft);
	}

	.link-destructive .link-icon {
		background: var(--danger-soft);
		color: var(--danger);
	}

	.link-destructive:hover {
		background: rgba(179, 36, 0, 0.04);
	}

	@media (prefers-reduced-motion: reduce) {
		.link-card { transition: none; }
	}
</style>
