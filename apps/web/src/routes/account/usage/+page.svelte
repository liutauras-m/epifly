<script lang="ts">
	import { ArrowUpRight } from 'lucide-svelte';
	import type { PageData } from './$types';

	export let data: PageData;
	const { usage, subscription } = data;

	const planKey = subscription?.plan_key ?? 'free';

	const limits: Record<string, { turns: number | null; tokens: number | null }> = {
		free:       { turns: 50, tokens: null },
		pro:        { turns: 500, tokens: null },
		team:       { turns: 2000, tokens: null },
		enterprise: { turns: null, tokens: null },
	};

	const limit = limits[planKey] ?? limits.free;

	function pct(used: number, max: number | null): number {
		if (!max) return 0;
		return Math.min(100, Math.round((used / max) * 100));
	}

	function fmt(n: number): string {
		return n.toLocaleString();
	}
</script>

<svelte:head>
	<title>Usage — ConusAI</title>
</svelte:head>

<div class="usage-page">
	<nav class="breadcrumb" aria-label="Breadcrumb">
		<a href="/account">Account</a>
		<span aria-hidden="true">›</span>
		<span>Usage</span>
	</nav>

	<h1>Usage</h1>
	<p class="period">Today (UTC)</p>

	<div class="meters">
		<!-- Agent Turns -->
		<div class="meter-card">
			<div class="meter-header">
				<span class="meter-label">Agent Turns</span>
				<span class="meter-value">
					{fmt(usage.agent_turns)}
					{#if limit.turns}/ {fmt(limit.turns)}{/if}
				</span>
			</div>
			{#if limit.turns}
				{@const p = pct(usage.agent_turns, limit.turns)}
				<div
					class="progress-bar"
					role="progressbar"
					aria-valuenow={usage.agent_turns}
					aria-valuemax={limit.turns}
					aria-label="Agent turns used"
				>
					<div
						class="progress-fill"
						class:warn={p >= 80 && p < 100}
						class:danger={p >= 100}
						style="width: {p}%"
					></div>
				</div>
				<p class="meter-hint">{fmt(limit.turns - usage.agent_turns)} remaining today</p>
			{:else}
				<p class="meter-hint unlimited">Unlimited on your plan</p>
			{/if}
		</div>

		<!-- Tokens -->
		<div class="meter-card">
			<div class="meter-header">
				<span class="meter-label">Tokens Used</span>
				<span class="meter-value">{fmt(usage.tokens)}</span>
			</div>
			<p class="meter-hint">Billed as usage (see invoices)</p>
		</div>

		<!-- Storage -->
		<div class="meter-card">
			<div class="meter-header">
				<span class="meter-label">Storage</span>
				<span class="meter-value">{usage.storage_gb.toFixed(2)} GB</span>
			</div>
			<p class="meter-hint">Workspace files and artifacts</p>
		</div>
	</div>

	{#if planKey === 'free'}
		<div class="upgrade-banner">
			<p>You're on the Free plan. Upgrade for more turns, tokens, and storage.</p>
			<a href="/account/billing" class="btn-upgrade">
				Upgrade Now
				<ArrowUpRight size={15} strokeWidth={1.75} aria-hidden="true" />
			</a>
		</div>
	{/if}
</div>

<style>
	.usage-page {
		max-width: 640px;
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
		font-family: var(--font-family-sans);
		font-size: 1.75rem;
		font-weight: 800;
		letter-spacing: -0.04em;
		color: var(--ink);
		margin-bottom: 0.25rem;
	}

	.period {
		font-family: var(--font-mono);
		font-size: 0.78rem;
		letter-spacing: 0.04em;
		color: var(--ink-3);
		text-transform: uppercase;
		margin-bottom: 1.5rem;
	}

	.meters {
		display: flex;
		flex-direction: column;
		gap: 0.875rem;
	}

	.meter-card {
		padding: 1.25rem;
		border: 1px solid var(--rule);
		border-radius: var(--radius-lg);
		background: var(--paper);
	}

	.meter-header {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		margin-bottom: 0.6rem;
	}

	.meter-label {
		font-family: var(--font-family-sans);
		font-weight: 600;
		font-size: 0.875rem;
		color: var(--ink);
	}

	.meter-value {
		font-family: var(--font-family-sans);
		font-size: 1.05rem;
		font-weight: 700;
		letter-spacing: -0.03em;
		color: var(--ink);
	}

	.progress-bar {
		height: 7px;
		background: var(--paper-3);
		border-radius: var(--radius-full);
		overflow: hidden;
		margin-bottom: 0.4rem;
	}

	.progress-fill {
		height: 100%;
		background: var(--ember);
		border-radius: var(--radius-full);
		transition: width 300ms cubic-bezier(0.4, 0, 0.2, 1),
		            background 180ms cubic-bezier(0.4, 0, 0.2, 1);
	}

	.progress-fill.warn   { background: #d97706; }
	.progress-fill.danger { background: var(--danger); }

	.meter-hint {
		font-family: var(--font-mono);
		font-size: 0.72rem;
		letter-spacing: 0.03em;
		color: var(--ink-3);
		margin: 0;
	}

	.meter-hint.unlimited { color: var(--success); }

	/* Upgrade banner */
	.upgrade-banner {
		margin-top: 1.75rem;
		padding: 1.25rem 1.5rem;
		background: var(--ember-soft);
		border: 1px solid var(--ember-glow);
		border-radius: var(--radius-lg);
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		flex-wrap: wrap;
	}

	.upgrade-banner p {
		margin: 0;
		font-size: 0.875rem;
		color: var(--ink-2);
		font-weight: 500;
	}

	.btn-upgrade {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.5rem 1.1rem;
		background: var(--ember);
		color: #fff;
		border: none;
		border-radius: var(--radius-md);
		font-family: var(--font-family-sans);
		font-weight: 600;
		text-decoration: none;
		white-space: nowrap;
		font-size: 0.875rem;
		transition: transform 120ms cubic-bezier(0.4, 0, 0.2, 1),
		            box-shadow 120ms cubic-bezier(0.4, 0, 0.2, 1);
		box-shadow: 0 4px 14px var(--ember-glow);
	}

	.btn-upgrade:hover {
		transform: translateY(-2px);
		box-shadow: 0 8px 20px var(--ember-glow);
	}

	.btn-upgrade:active { transform: scale(0.97); }

	.btn-upgrade:focus-visible {
		outline: none;
		box-shadow: 0 0 0 3px var(--ember-glow);
	}

	@media (prefers-reduced-motion: reduce) {
		.progress-fill,
		.btn-upgrade { transition: none; }
	}
</style>
