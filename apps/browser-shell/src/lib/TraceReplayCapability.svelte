<script lang="ts">
	import type { CapabilityCard } from '@conusai/types';
	import { invoke } from '@tauri-apps/api/core';

	let { card }: { card: CapabilityCard } = $props();

	let traceNodeId = $state('');
	let dryRun = $state(false);
	let status = $state<'idle' | 'running' | 'done' | 'error'>('idle');
	let resultMsg = $state('');

	async function handleReplay(e: SubmitEvent) {
		e.preventDefault();
		if (!traceNodeId.trim()) return;
		status = 'running';
		resultMsg = '';
		try {
			const result = await invoke<string>('upload_trace_cmd', {
				traceNodeId: traceNodeId.trim(),
				dryRun,
			});
			status = 'done';
			resultMsg = result ?? 'Replay queued.';
		} catch (e) {
			status = 'error';
			resultMsg = String(e);
		}
	}
</script>

<div class="trace-replay">
	<h3 class="card-title">Trace Replay — <code>{card.name}</code></h3>
	<form class="replay-form" onsubmit={handleReplay}>
		<label>
			Trace node ID
			<input
				type="text"
				bind:value={traceNodeId}
				placeholder="01J…"
				disabled={status === 'running'}
			/>
		</label>
		<label class="checkbox-label">
			<input type="checkbox" bind:checked={dryRun} disabled={status === 'running'} />
			Dry run (validate only)
		</label>
		<button type="submit" disabled={status === 'running' || !traceNodeId.trim()}>
			{status === 'running' ? 'Replaying…' : 'Replay'}
		</button>
	</form>

	{#if status === 'done'}
		<p class="result success">{resultMsg}</p>
	{:else if status === 'error'}
		<p class="result error" role="alert">{resultMsg}</p>
	{/if}
</div>

<style>
	.trace-replay {
		padding: var(--s-4);
		border: 1px solid var(--rule);
		border-radius: var(--r-sm);
		background: var(--paper-2);
	}

	.card-title {
		font-size: var(--t-body);
		font-weight: 600;
		color: var(--ink);
		margin: 0 0 var(--s-3);
	}

	.replay-form {
		display: flex;
		flex-direction: column;
		gap: var(--s-2);
	}

	.replay-form label {
		display: flex;
		flex-direction: column;
		gap: var(--s-1);
		font-size: var(--t-label);
		color: var(--ink-2);
	}

	.checkbox-label {
		flex-direction: row !important;
		align-items: center;
		gap: var(--s-2) !important;
	}

	.replay-form input[type="text"] {
		padding: var(--s-1) var(--s-2);
		border: 1px solid var(--rule);
		border-radius: var(--r-sm);
		background: var(--paper);
		color: var(--ink);
		font-family: var(--font-mono);
		font-size: var(--t-label);
	}

	.replay-form button {
		align-self: flex-start;
		padding: var(--s-1) var(--s-3);
		background: var(--ember);
		color: var(--paper);
		border: none;
		border-radius: var(--r-sm);
		font-size: var(--t-label);
		font-weight: 600;
		cursor: pointer;
	}

	.replay-form button:disabled { opacity: 0.5; cursor: default; }

	.result {
		margin: var(--s-2) 0 0;
		font-size: var(--t-label);
		font-family: var(--font-mono);
	}

	.result.success { color: var(--success); }
	.result.error { color: var(--ember); }
</style>
