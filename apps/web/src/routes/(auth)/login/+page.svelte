<script lang="ts">
  import { page } from "$app/state";

  const returnTo = $derived(encodeURIComponent(page.url.searchParams.get("returnTo") ?? "/"));
  const loginHref = $derived(`/auth/login?returnTo=${returnTo}`);
</script>

<svelte:head>
  <title>Account · Epifly</title>
</svelte:head>

<div class="flex min-h-svh items-center justify-center p-6">
  <div class="w-full max-w-sm space-y-4 text-center">
    <h1 class="text-2xl font-semibold tracking-tight">Sign in to Epifly</h1>
    <a
      href={loginHref}
      class="inline-flex w-full items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
    >
      Continue with Zitadel
    </a>
  </div>
</div>
