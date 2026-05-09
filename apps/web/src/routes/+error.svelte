<script lang="ts">
	import { page } from '$app/stores';
</script>

<svelte:head>
	<title>Error {$page.status} · ConusAI</title>
</svelte:head>

<div class="error-screen">
	<div class="error-inner">
		<span class="error-code label-mono">{$page.status}</span>
		<h1 class="error-title">{$page.error?.message ?? 'Something went wrong'}</h1>
		<a class="error-back" href="/">Back to Workshop</a>
	</div>
</div>

<style>
	.error-screen {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 100dvh;
		padding: 2rem;
		background: var(--bg, #f5f0eb);
	}
	.error-inner {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.75rem;
		text-align: center;
	}
	.error-code {
		font-size: 0.75rem;
		letter-spacing: 0.1em;
		opacity: 0.5;
	}
	.error-title {
		font-size: 1.25rem;
		font-weight: 600;
		margin: 0;
	}
	.error-back {
		margin-top: 0.5rem;
		font-size: 0.875rem;
		color: var(--accent, #0d9488);
		text-decoration: none;
	}
	.error-back:hover {
		text-decoration: underline;
	}
</style>
