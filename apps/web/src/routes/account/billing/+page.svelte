<script lang="ts">
	import type { ActionData, PageData } from './$types';
	import { enhance } from '$app/forms';

	export let data: PageData;
	export let form: ActionData;

	const { plans, subscription } = data;

	$: currentPlan = subscription?.plan_key ?? 'free';
	$: isActive = subscription?.status === 'active' || subscription?.status === 'trialing';

	const planIcons: Record<string, string> = {
		free: '🆓',
		pro: '⚡',
		team: '👥',
		enterprise: '🏢',
	};

	const planColors: Record<string, string> = {
		free: 'plan-free',
		pro: 'plan-pro',
		team: 'plan-team',
		enterprise: 'plan-enterprise',
	};
</script>

<svelte:head>
	<title>Billing — ConusAI</title>
</svelte:head>

<div class="billing-page">
	<nav class="breadcrumb">
		<a href="/account">Account</a>
		<span>›</span>
		<span>Billing</span>
	</nav>

	<h1>Billing &amp; Plans</h1>

	{#if form?.error}
		<div class="error-banner">{form.error}</div>
	{/if}

	<!-- Current plan summary -->
	{#if subscription}
		<section class="current-plan">
			<h2>Current Plan</h2>
			<div class="plan-summary">
				<span class="plan-badge {planColors[currentPlan] ?? 'plan-free'}">
					{planIcons[currentPlan] ?? '📦'} {currentPlan.toUpperCase()}
				</span>
				<span class="status-{subscription.status}">{subscription.status}</span>
				{#if subscription.current_period_end}
					<span class="period">
						Renews {new Date(subscription.current_period_end).toLocaleDateString()}
					</span>
				{/if}
			</div>
			<div class="portal-actions">
				<form method="POST" action="?/portal" use:enhance>
					<button class="btn-secondary" type="submit">Manage Billing</button>
				</form>
				{#if isActive && currentPlan !== 'free'}
					<form method="POST" action="?/cancel" use:enhance>
						<button class="btn-danger" type="submit">Cancel Subscription</button>
					</form>
				{/if}
			</div>
		</section>
	{/if}

	<!-- Plan cards -->
	<section class="plans-grid">
		<h2>Available Plans</h2>
		<div class="grid">
			{#each plans as plan}
				<div class="plan-card {plan.key === currentPlan ? 'current' : ''}">
					<div class="plan-header">
						<span class="plan-icon">{planIcons[plan.key] ?? '📦'}</span>
						<h3>{plan.display_name}</h3>
						<div class="price">
							{#if plan.monthly_price_cents === 0}
								<strong>Free</strong>
							{:else}
								<strong>${(plan.monthly_price_cents / 100).toFixed(0)}</strong>
								<span>/mo</span>
							{/if}
						</div>
					</div>
					<ul class="features">
						{#if plan.max_turns_per_day}
							<li>{plan.max_turns_per_day.toLocaleString()} agent turns/day</li>
						{:else}
							<li>Unlimited agent turns</li>
						{/if}
						{#if plan.max_storage_gb}
							<li>{plan.max_storage_gb} GB storage</li>
						{:else}
							<li>Unlimited storage</li>
						{/if}
						<li>{plan.max_tokens.toLocaleString()} max tokens/request</li>
						<li>{plan.rate_limit_rpm} requests/min</li>
					</ul>
					{#if plan.key !== currentPlan && plan.key !== 'enterprise'}
						<form method="POST" action="?/upgrade" use:enhance>
							<input type="hidden" name="plan_key" value={plan.key} />
							<button class="btn-primary upgrade-btn" type="submit">
								{plan.monthly_price_cents > 0 ? 'Upgrade' : 'Downgrade'}
							</button>
						</form>
					{:else if plan.key === currentPlan}
						<div class="current-tag">Current Plan</div>
					{:else}
						<a href="mailto:sales@conusai.com" class="btn-secondary contact-btn">
							Contact Sales
						</a>
					{/if}
				</div>
			{/each}
		</div>
	</section>

	<!-- Invoices -->
	<section class="invoices">
		<h2>Invoices</h2>
		<span class="muted">View and download invoices from the </span>
		<form method="POST" action="?/portal" use:enhance style="display:inline">
			<button class="link-btn" type="submit">billing portal</button>
		</form><span class="muted">.</span>
	</section>
</div>

<style>
	.billing-page {
		max-width: 800px;
		margin: 0 auto;
		padding: 2rem 1rem;
	}
	.breadcrumb {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		margin-bottom: 1rem;
		font-size: 0.875rem;
		color: #6b7280;
	}
	.breadcrumb a { color: #6366f1; text-decoration: none; }
	h1 { font-size: 1.75rem; font-weight: 700; margin-bottom: 1.5rem; }
	h2 { font-size: 1.1rem; font-weight: 600; margin-bottom: 0.75rem; }
	.error-banner {
		background: #fee2e2;
		color: #dc2626;
		padding: 0.75rem 1rem;
		border-radius: 8px;
		margin-bottom: 1rem;
	}
	.current-plan {
		padding: 1.25rem;
		border: 1px solid #e5e7eb;
		border-radius: 12px;
		margin-bottom: 2rem;
	}
	.plan-summary {
		display: flex;
		gap: 0.75rem;
		align-items: center;
		margin-bottom: 1rem;
		flex-wrap: wrap;
	}
	.plan-badge {
		padding: 0.25rem 0.75rem;
		border-radius: 999px;
		font-size: 0.8rem;
		font-weight: 600;
	}
	.plan-free { background: #f3f4f6; color: #6b7280; }
	.plan-pro { background: #ede9fe; color: #7c3aed; }
	.plan-team { background: #dbeafe; color: #1d4ed8; }
	.plan-enterprise { background: #fef3c7; color: #92400e; }
	.status-active, .status-trialing { color: #16a34a; font-size: 0.875rem; }
	.status-past_due, .status-canceled { color: #dc2626; font-size: 0.875rem; }
	.period { color: #6b7280; font-size: 0.875rem; }
	.portal-actions { display: flex; gap: 0.75rem; }
	.plans-grid { margin-bottom: 2rem; }
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
		gap: 1rem;
	}
	.plan-card {
		padding: 1.25rem;
		border: 1px solid #e5e7eb;
		border-radius: 12px;
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
	}
	.plan-card.current { border-color: #6366f1; box-shadow: 0 0 0 2px #c7d2fe; }
	.plan-header { display: flex; flex-direction: column; gap: 0.25rem; }
	.plan-icon { font-size: 1.5rem; }
	.plan-header h3 { font-weight: 600; margin: 0; }
	.price { display: flex; align-items: baseline; gap: 0.25rem; }
	.price strong { font-size: 1.5rem; font-weight: 700; }
	.price span { color: #6b7280; font-size: 0.875rem; }
	.features { list-style: none; padding: 0; margin: 0; font-size: 0.85rem; color: #374151; }
	.features li::before { content: '✓ '; color: #16a34a; }
	.current-tag {
		text-align: center;
		font-size: 0.8rem;
		color: #6366f1;
		font-weight: 600;
		padding: 0.375rem;
	}
	.btn-primary {
		display: block;
		width: 100%;
		padding: 0.5rem;
		background: #6366f1;
		color: #fff;
		border: none;
		border-radius: 8px;
		font-weight: 600;
		cursor: pointer;
	}
	.btn-primary:hover { background: #4f46e5; }
	.btn-secondary {
		padding: 0.5rem 1rem;
		background: #f3f4f6;
		color: #374151;
		border: 1px solid #d1d5db;
		border-radius: 8px;
		font-weight: 500;
		cursor: pointer;
	}
	.btn-danger {
		padding: 0.5rem 1rem;
		background: #fee2e2;
		color: #dc2626;
		border: 1px solid #fecaca;
		border-radius: 8px;
		font-weight: 500;
		cursor: pointer;
	}
	.contact-btn {
		display: block;
		text-align: center;
		padding: 0.5rem;
		text-decoration: none;
		background: #f3f4f6;
		color: #374151;
		border: 1px solid #d1d5db;
		border-radius: 8px;
		font-size: 0.875rem;
	}
	.upgrade-btn { margin-top: auto; }
	.invoices { color: #374151; }
	.muted { color: #6b7280; font-size: 0.875rem; }
	.link-btn {
		background: none;
		border: none;
		color: #6366f1;
		cursor: pointer;
		font-size: inherit;
		padding: 0;
		text-decoration: underline;
	}
</style>
