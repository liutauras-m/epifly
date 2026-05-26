<script lang="ts">
	import { AppSafeArea, ChatComposer } from "@epifly/ui";
</script>

<svelte:head>
	<title>Epifly Chat</title>
</svelte:head>

<AppSafeArea class="flex min-h-0 flex-1 flex-col">
	<section class="flex min-h-0 flex-1 items-center justify-center px-6 py-16">
		<div class="w-full max-w-xl">
			<ChatComposer disabled={true} placeholder="How can Epifly help?" />
		</div>
	</section>
</AppSafeArea>
