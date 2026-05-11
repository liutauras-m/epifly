<script lang="ts">
	import type { ConusSdk } from '@conusai/sdk';

	let {
		sdk,
		onAuthenticated,
	}: {
		sdk: ConusSdk;
		onAuthenticated?: (token: string) => void;
	} = $props();

	let deviceToken = $state('');
	let status = $state<'idle' | 'submitting' | 'error'>('idle');
	let errorMsg = $state('');

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (!deviceToken.trim()) return;
		status = 'submitting';
		errorMsg = '';
		const result = await sdk.auth.verifyDeviceToken(deviceToken.trim());
		if (result.error) {
			status = 'error';
			errorMsg = result.error.message;
		} else {
			status = 'idle';
			onAuthenticated?.(deviceToken.trim());
		}
	}
</script>

<div class="login-panel">
	<div class="login-card">
		<h1 class="login-title">ConusAI Browser Shell</h1>
		<p class="login-sub">Enter your device token to connect this shell to your workspace.</p>

		<form onsubmit={handleSubmit} class="login-form">
			<label for="token-input" class="label">Device token</label>
			<input
				id="token-input"
				type="password"
				class="token-input"
				bind:value={deviceToken}
				placeholder="dv_…"
				autocomplete="off"
				spellcheck={false}
				disabled={status === 'submitting'}
			/>

			{#if status === 'error'}
				<p class="error-msg" role="alert">{errorMsg}</p>
			{/if}

			<button type="submit" class="submit-btn" disabled={status === 'submitting' || !deviceToken.trim()}>
				{status === 'submitting' ? 'Connecting…' : 'Connect'}
			</button>
		</form>

		<p class="hint">
			Tokens are provisioned by your ConusAI admin.<br>
			They are stored locally in Stronghold and never sent to third parties.
		</p>
	</div>
</div>

<style>
	.login-panel {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		background: var(--paper);
	}

	.login-card {
		width: 360px;
		padding: var(--s-8);
		border: 1px solid var(--rule);
		border-radius: var(--r-lg);
		background: var(--paper-2);
		display: flex;
		flex-direction: column;
		gap: var(--s-4);
	}

	.login-title {
		font-family: var(--font-display);
		font-size: var(--t-h2);
		color: var(--ink);
		margin: 0;
	}

	.login-sub {
		font-size: var(--t-body);
		color: var(--ink-3);
		margin: 0;
		line-height: 1.5;
	}

	.login-form {
		display: flex;
		flex-direction: column;
		gap: var(--s-2);
	}

	.label {
		font-size: var(--t-label);
		font-weight: 600;
		color: var(--ink-2);
	}

	.token-input {
		padding: var(--s-2) var(--s-3);
		border: 1px solid var(--rule);
		border-radius: var(--r-sm);
		background: var(--paper);
		color: var(--ink);
		font-family: var(--font-mono);
		font-size: var(--t-body);
		width: 100%;
		box-sizing: border-box;
	}

	.token-input:focus {
		outline: 2px solid var(--ember);
		outline-offset: 1px;
	}

	.error-msg {
		font-size: var(--t-label);
		color: var(--ember);
		margin: 0;
	}

	.submit-btn {
		padding: var(--s-2) var(--s-4);
		background: var(--ember);
		color: var(--paper);
		border: none;
		border-radius: var(--r-sm);
		font-size: var(--t-body);
		font-weight: 600;
		cursor: pointer;
		align-self: flex-start;
		margin-top: var(--s-2);
	}

	.submit-btn:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.hint {
		font-size: var(--t-label);
		color: var(--ink-3);
		margin: 0;
		line-height: 1.6;
	}
</style>
