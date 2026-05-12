<script lang="ts">
	import type { PageData } from './$types';

	export let data: PageData;

	const { user, subscription, authProvider } = data;

	$: planLabel = subscription?.plan_key ?? user?.plan ?? 'free';
	$: statusLabel = subscription?.status ?? 'active';
	$: isPaid = ['pro', 'team', 'enterprise'].includes(planLabel.toLowerCase());
</script>

<svelte:head>
	<title>Account — ConusAI</title>
</svelte:head>

<div class="account-page">
	<header class="account-header">
		<h1>Account</h1>
	</header>

	<section class="profile-card">
		<div class="avatar">{(user?.name ?? '?')[0].toUpperCase()}</div>
		<div class="profile-info">
			<p class="name">{user?.name ?? 'Unknown'}</p>
			<span class="plan-badge plan-{planLabel.toLowerCase()}">{planLabel.toUpperCase()}</span>
			{#if statusLabel !== 'active'}
				<span class="status-badge status-{statusLabel}">{statusLabel}</span>
			{/if}
		</div>
	</section>

	<section class="links">
		<a href="/account/billing" class="link-card">
			<span class="icon">💳</span>
			<div>
				<strong>Billing &amp; Plans</strong>
				<p>Manage your subscription, upgrade, or view invoices.</p>
			</div>
		</a>
		<a href="/account/usage" class="link-card">
			<span class="icon">📊</span>
			<div>
				<strong>Usage</strong>
				<p>View agent turns, token consumption, and storage.</p>
			</div>
		</a>
		{#if authProvider === 'zitadel'}
			<a href="/auth/logout" class="link-card link-destructive">
				<span class="icon">🚪</span>
				<div>
					<strong>Sign out</strong>
					<p>End your session.</p>
				</div>
			</a>
		{:else}
			<a href="/logout" class="link-card link-destructive">
				<span class="icon">🚪</span>
				<div>
					<strong>Sign out</strong>
					<p>End your session.</p>
				</div>
			</a>
		{/if}
	</section>
</div>

<style>
	.account-page {
		max-width: 640px;
		margin: 0 auto;
		padding: 2rem 1rem;
	}
	.account-header h1 {
		font-size: 1.75rem;
		font-weight: 700;
		margin-bottom: 1.5rem;
	}
	.profile-card {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 1.25rem;
		border: 1px solid #e5e7eb;
		border-radius: 12px;
		margin-bottom: 1.5rem;
	}
	.avatar {
		width: 48px;
		height: 48px;
		border-radius: 50%;
		background: #6366f1;
		color: #fff;
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 700;
		font-size: 1.25rem;
	}
	.profile-info {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}
	.name { font-weight: 600; }
	.plan-badge, .status-badge {
		display: inline-block;
		padding: 0.125rem 0.5rem;
		border-radius: 999px;
		font-size: 0.7rem;
		font-weight: 600;
	}
	.plan-free { background: #f3f4f6; color: #6b7280; }
	.plan-pro { background: #ede9fe; color: #7c3aed; }
	.plan-team, .plan-enterprise { background: #fef3c7; color: #92400e; }
	.status-past_due, .status-canceled { background: #fee2e2; color: #dc2626; }
	.links { display: flex; flex-direction: column; gap: 0.75rem; }
	.link-card {
		display: flex;
		align-items: flex-start;
		gap: 1rem;
		padding: 1rem 1.25rem;
		border: 1px solid #e5e7eb;
		border-radius: 12px;
		text-decoration: none;
		color: inherit;
		transition: background 0.15s;
	}
	.link-card:hover { background: #f9fafb; }
	.link-card .icon { font-size: 1.5rem; }
	.link-card strong { display: block; font-weight: 600; }
	.link-card p { margin: 0; font-size: 0.875rem; color: #6b7280; }
	.link-destructive { border-color: #fecaca; }
	.link-destructive:hover { background: #fff5f5; }
</style>
