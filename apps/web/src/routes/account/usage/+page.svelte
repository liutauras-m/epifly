<script lang="ts">
	import type { PageData } from './$types';

	export let data: PageData;
	const { usage, subscription } = data;

	const planKey = subscription?.plan_key ?? 'free';

	const limits: Record<string, { turns: number | null; tokens: number | null }> = {
		free: { turns: 50, tokens: null },
		pro: { turns: 500, tokens: null },
		team: { turns: 2000, tokens: null },
		enterprise: { turns: null, tokens: null },
	};

	const limit = limits[planKey] ?? limits.free;

	function pct(used: number, max: number | null): number {
		if (!max) return 0;
		return Math.min(100, Math.round((used / max) * 100));
	}
</script>

<svelte:head>
	<title>Usage — ConusAI</title>
</svelte:head>

<div class="usage-page">
	<nav class="breadcrumb">
		<a href="/account">Account</a>
		<span>›</span>
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
					{usage.agent_turns.toLocaleString()}
					{#if limit.turns}/ {limit.turns.toLocaleString()}{/if}
				</span>
			</div>
			{#if limit.turns}
				<div class="progress-bar">
					<div
						class="progress-fill {pct(usage.agent_turns, limit.turns) >= 90 ? 'danger' : ''}"
						style="width: {pct(usage.agent_turns, limit.turns)}%"
					/>
				</div>
				<p class="meter-hint">
					{limit.turns - usage.agent_turns} remaining today
				</p>
			{:else}
				<p class="meter-hint unlimited">Unlimited on your plan</p>
			{/if}
		</div>

		<!-- Tokens -->
		<div class="meter-card">
			<div class="meter-header">
				<span class="meter-label">Tokens Used</span>
				<span class="meter-value">{usage.tokens.toLocaleString()}</span>
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
			<a href="/account/billing" class="btn-primary">Upgrade Now</a>
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
		font-size: 0.875rem;
		color: #6b7280;
	}
	.breadcrumb a { color: #6366f1; text-decoration: none; }
	h1 { font-size: 1.75rem; font-weight: 700; margin-bottom: 0.25rem; }
	.period { color: #6b7280; font-size: 0.875rem; margin-bottom: 1.5rem; }
	.meters { display: flex; flex-direction: column; gap: 1rem; }
	.meter-card {
		padding: 1.25rem;
		border: 1px solid #e5e7eb;
		border-radius: 12px;
	}
	.meter-header {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		margin-bottom: 0.5rem;
	}
	.meter-label { font-weight: 600; }
	.meter-value { font-size: 1.1rem; font-weight: 700; color: #111; }
	.progress-bar {
		height: 8px;
		background: #f3f4f6;
		border-radius: 999px;
		overflow: hidden;
		margin-bottom: 0.375rem;
	}
	.progress-fill {
		height: 100%;
		background: #6366f1;
		border-radius: 999px;
		transition: width 0.3s ease;
	}
	.progress-fill.danger { background: #dc2626; }
	.meter-hint { font-size: 0.8rem; color: #6b7280; margin: 0; }
	.meter-hint.unlimited { color: #16a34a; }
	.upgrade-banner {
		margin-top: 2rem;
		padding: 1.25rem;
		background: #f5f3ff;
		border: 1px solid #c4b5fd;
		border-radius: 12px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		flex-wrap: wrap;
	}
	.upgrade-banner p { margin: 0; font-size: 0.9rem; color: #4c1d95; }
	.btn-primary {
		padding: 0.5rem 1.25rem;
		background: #6366f1;
		color: #fff;
		border: none;
		border-radius: 8px;
		font-weight: 600;
		text-decoration: none;
		white-space: nowrap;
		font-size: 0.875rem;
	}
</style>
