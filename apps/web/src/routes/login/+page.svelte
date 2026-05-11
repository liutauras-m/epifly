<script lang="ts">
	import type { PageData, ActionData } from './$types';
	import logoDark from '@conusai/ui/assets/images/conusai-logo-darkmode.png';
	let { data, form }: { data: PageData; form: ActionData } = $props();
</script>

<svelte:head><title>Enter · ConusAI</title></svelte:head>

<div class="login-page">
	<aside class="login-poster">
		<div class="poster-brand">
			<img src={logoDark} alt="ConusAI" class="poster-logo">
		</div>
		<h1 class="poster-tagline">An <em>agent workshop</em> for operators who build with intent.</h1>
		<div class="poster-meta">
			<div>v0.4 · {new Date().getFullYear()}</div>
			<div>Forge · stream · inspect</div>
		</div>
	</aside>

	<section class="login-form-wrap">
		<form class="login-form" method="POST">
			<div class="login-eyebrow">{data.greeting} · ConusAI workshop</div>
			<h2 class="login-h1">Enter the workshop.</h2>

			<div class="field">
				<label for="name">Operator name</label>
				<input id="name" name="name" type="text" required maxlength="60"
					value={form?.name ?? 'John Smith'} autocomplete="off" autofocus>
			</div>

			<div class="field">
				<label>Plan tier</label>
				<div class="plan-row">
					<input type="radio" id="p-free" name="plan" value="free">
					<label for="p-free">Free</label>
					<input type="radio" id="p-pro" name="plan" value="pro">
					<label for="p-pro">Pro</label>
					<input type="radio" id="p-ent" name="plan" value="enterprise" checked>
					<label for="p-ent">Enterprise</label>
				</div>
			</div>

			{#if form?.error}
				<div class="empty-hint" style="color:var(--danger);margin-bottom:var(--s-3)">{form.error}</div>
			{/if}

			<button class="submit" type="submit">
				Begin
				<svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="square">
					<path d="M5 12h14M12 5l7 7-7 7"/>
				</svg>
			</button>
		</form>
	</section>
</div>
