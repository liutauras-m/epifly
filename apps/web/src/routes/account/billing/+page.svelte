<script lang="ts">
	import { enhance } from '$app/forms';
	import { Layers, Zap, Users, Building2, Check, ArrowUpRight } from 'lucide-svelte';
	import type { ActionData, PageData } from './$types';

	export let data: PageData;
	export let form: ActionData;

	const { plans, subscription } = data;

	$: currentPlan = subscription?.plan_key ?? 'free';
	$: isActive = subscription?.status === 'active' || subscription?.status === 'trialing';

	const planIcons: Record<string, typeof Zap> = {
		free: Layers, pro: Zap, team: Users, enterprise: Building2,
	};
</script>

<svelte:head>
	<title>Billing — ConusAI</title>
</svelte:head>

<div class="billing-page">
	<nav class="breadcrumb" aria-label="Breadcrumb">
		<a href="/account">Account</a>
		<span aria-hidden="true">›</span>
		<span>Billing</span>
	</nav>

	<h1>Billing &amp; Plans</h1>

	{#if form?.error}
		<div class="error-banner" role="alert">{form.error}</div>
	{/if}

	<!-- Current plan summary -->
	{#if subscription}
		<section class="current-plan">
			<h2>Current Plan</h2>
			<div class="plan-summary">
				<span class="plan-badge badge-{currentPlan}">{currentPlan.toUpperCase()}</span>
				<span class="plan-status status-{subscription.status}">{subscription.status.replace('_', ' ')}</span>
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
	<section class="plans-section">
		<h2>Available Plans</h2>
		<div class="plans-grid">
			{#each plans as plan}
				{@const isCurrent = plan.key === currentPlan}
				{@const Icon = planIcons[plan.key] ?? Layers}
				<div class="plan-card" class:current={isCurrent}>
					<div class="plan-header">
						<span class="plan-icon" aria-hidden="true">
							<svelte:component this={Icon} size={20} strokeWidth={1.5} />
						</span>
						<h3>{plan.display_name}</h3>
						<div class="price">
							{#if plan.monthly_price_cents === 0}
								<strong>Free</strong>
							{:else}
								<strong>${(plan.monthly_price_cents / 100).toFixed(0)}</strong>
								<span class="per-mo">/mo</span>
							{/if}
						</div>
					</div>

					<ul class="features">
						{#if plan.max_turns_per_day}
							<li><Check size={12} strokeWidth={2} aria-hidden="true" />{plan.max_turns_per_day.toLocaleString()} agent turns/day</li>
						{:else}
							<li><Check size={12} strokeWidth={2} aria-hidden="true" />Unlimited agent turns</li>
						{/if}
						{#if plan.max_storage_gb}
							<li><Check size={12} strokeWidth={2} aria-hidden="true" />{plan.max_storage_gb} GB storage</li>
						{:else}
							<li><Check size={12} strokeWidth={2} aria-hidden="true" />Unlimited storage</li>
						{/if}
						<li><Check size={12} strokeWidth={2} aria-hidden="true" />{plan.max_tokens.toLocaleString()} tokens/request</li>
						<li><Check size={12} strokeWidth={2} aria-hidden="true" />{plan.rate_limit_rpm} requests/min</li>
					</ul>

					{#if plan.key !== currentPlan && plan.key !== 'enterprise'}
						<form method="POST" action="?/upgrade" use:enhance>
							<input type="hidden" name="plan_key" value={plan.key} />
							<button class="btn-primary" type="submit">
								{plan.monthly_price_cents > 0 ? 'Upgrade' : 'Downgrade'}
								<ArrowUpRight size={14} strokeWidth={1.75} aria-hidden="true" />
							</button>
						</form>
					{:else if isCurrent}
						<div class="current-tag">Current Plan</div>
					{:else}
						<a href="mailto:sales@conusai.com" class="btn-contact">Contact Sales</a>
					{/if}
				</div>
			{/each}
		</div>
	</section>

	<!-- Invoices -->
	<section class="invoices">
		<h2>Invoices</h2>
		<p class="invoices-hint">
			View and download invoices from the billing portal.
		</p>
		<form method="POST" action="?/portal" use:enhance>
			<button class="link-btn" type="submit">Open billing portal</button>
		</form>
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
		font-family: var(--font-mono);
		font-size: 0.78rem;
		letter-spacing: 0.04em;
		color: var(--ink-3);
	}

	.breadcrumb a {
		color: var(--ember);
		text-decoration: none;
		font-weight: 500;
	}

	.breadcrumb a:hover { text-decoration: underline; }

	h1 {
		font-family: var(--font-display);
		font-size: 1.75rem;
		font-weight: 800;
		letter-spacing: -0.04em;
		color: var(--ink);
		margin-bottom: 1.5rem;
	}

	h2 {
		font-family: var(--font-display);
		font-size: 1rem;
		font-weight: 700;
		letter-spacing: -0.02em;
		color: var(--ink);
		margin-bottom: 0.75rem;
	}

	.error-banner {
		background: var(--danger-soft);
		color: var(--danger);
		padding: 0.75rem 1rem;
		border-radius: var(--r-md);
		border: 1px solid rgba(179, 36, 0, 0.28);
		margin-bottom: 1rem;
		font-size: 0.875rem;
	}

	/* Current plan */
	.current-plan {
		padding: 1.25rem;
		border: 1px solid var(--rule);
		border-radius: var(--r-lg);
		margin-bottom: 2rem;
		background: var(--paper);
	}

	.plan-summary {
		display: flex;
		gap: 0.75rem;
		align-items: center;
		margin-bottom: 1rem;
		flex-wrap: wrap;
	}

	.plan-badge {
		padding: 0.12rem 0.55rem;
		border-radius: var(--r-full);
		font-family: var(--font-mono);
		font-size: 0.65rem;
		font-weight: 700;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.badge-free       { background: var(--paper-2); color: var(--ink-3); border: 1px solid var(--rule); }
	.badge-pro,
	.badge-enterprise { background: var(--ember-soft); color: var(--ember); border: 1px solid var(--ember-glow); }
	.badge-team       { background: var(--cyan-soft); color: var(--cyan); border: 1px solid rgba(0,212,255,0.28); }

	.plan-status {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		letter-spacing: 0.04em;
	}

	.status-active, .status-trialing { color: var(--success); }
	.status-past_due, .status-canceled { color: var(--danger); }

	.period {
		font-family: var(--font-mono);
		font-size: 0.75rem;
		color: var(--ink-3);
	}

	.portal-actions {
		display: flex;
		gap: 0.75rem;
	}

	/* Plan cards grid */
	.plans-section { margin-bottom: 2rem; }

	.plans-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
		gap: 1rem;
	}

	.plan-card {
		padding: 1.25rem;
		border: 1px solid var(--rule);
		border-radius: var(--r-lg);
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		background: var(--paper);
		transition: box-shadow 180ms cubic-bezier(0.4, 0, 0.2, 1);
	}

	.plan-card.current {
		border-color: var(--ember);
		box-shadow: 0 0 0 2px var(--ember-glow);
	}

	.plan-card:not(.current):hover {
		box-shadow: 0 6px 20px rgba(17, 17, 17, 0.07);
	}

	.plan-header { display: flex; flex-direction: column; gap: 0.25rem; }

	.plan-icon {
		display: inline-flex;
		color: var(--ember);
		margin-bottom: 0.1rem;
	}

	.plan-header h3 {
		font-family: var(--font-display);
		font-weight: 700;
		letter-spacing: -0.02em;
		font-size: 0.95rem;
		margin: 0;
		color: var(--ink);
	}

	.price { display: flex; align-items: baseline; gap: 0.2rem; }

	.price strong {
		font-family: var(--font-display);
		font-size: 1.4rem;
		font-weight: 800;
		letter-spacing: -0.04em;
		color: var(--ink);
	}

	.per-mo { font-size: 0.8rem; color: var(--ink-3); }

	.features {
		list-style: none;
		padding: 0;
		margin: 0;
		font-size: 0.82rem;
		color: var(--ink-2);
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
	}

	.features li {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}

	.features li :global(svg) { color: var(--success); flex-shrink: 0; }

	.current-tag {
		text-align: center;
		font-family: var(--font-mono);
		font-size: 0.68rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		color: var(--ember);
		font-weight: 600;
		padding: 0.4rem;
		border-top: 1px solid var(--ember-soft);
	}

	.btn-primary {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 0.3rem;
		width: 100%;
		padding: 0.5rem;
		background: var(--ember);
		color: #fff;
		border: none;
		border-radius: var(--r-md);
		font-family: var(--font-body);
		font-weight: 600;
		font-size: 0.875rem;
		cursor: pointer;
		box-shadow: 0 4px 12px var(--ember-glow);
		transition: transform 120ms cubic-bezier(0.4, 0, 0.2, 1),
		            box-shadow 120ms cubic-bezier(0.4, 0, 0.2, 1);
	}

	.btn-primary:hover {
		transform: translateY(-1px);
		box-shadow: 0 6px 16px var(--ember-glow);
	}

	.btn-primary:active { transform: scale(0.97); }

	.btn-secondary {
		padding: 0.45rem 1rem;
		background: transparent;
		color: var(--ink-2);
		border: 1px solid var(--seam);
		border-radius: var(--r-md);
		font-family: var(--font-body);
		font-weight: 500;
		font-size: 0.875rem;
		cursor: pointer;
		transition: background 120ms cubic-bezier(0.4, 0, 0.2, 1);
	}

	.btn-secondary:hover { background: var(--paper-2); }

	.btn-danger {
		padding: 0.45rem 1rem;
		background: transparent;
		color: var(--danger);
		border: 1px solid rgba(179, 36, 0, 0.28);
		border-radius: var(--r-md);
		font-family: var(--font-body);
		font-weight: 500;
		font-size: 0.875rem;
		cursor: pointer;
		transition: background 120ms cubic-bezier(0.4, 0, 0.2, 1);
	}

	.btn-danger:hover { background: var(--danger-soft); }

	.btn-contact {
		display: block;
		text-align: center;
		padding: 0.5rem;
		background: transparent;
		color: var(--ink-2);
		border: 1px solid var(--seam);
		border-radius: var(--r-md);
		text-decoration: none;
		font-size: 0.875rem;
		font-weight: 500;
		transition: background 120ms cubic-bezier(0.4, 0, 0.2, 1);
	}

	.btn-contact:hover { background: var(--paper-2); }

	/* Invoices */
	.invoices { color: var(--ink-2); }

	.invoices-hint {
		font-size: 0.875rem;
		color: var(--ink-3);
		margin: 0;
	}

	.link-btn {
		background: none;
		border: none;
		color: var(--ember);
		cursor: pointer;
		font-size: inherit;
		padding: 0;
		text-decoration: underline;
		font-family: inherit;
	}

	.link-btn:hover { color: var(--ember-2); }

	@media (prefers-reduced-motion: reduce) {
		.plan-card,
		.btn-primary,
		.btn-secondary,
		.btn-danger { transition: none; }
	}
</style>
