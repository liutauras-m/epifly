<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { getSdkContext, setWebAccessToken } from "@epifly/features";
  import { AuthOnboardingPanel } from "@epifly/ui";

  const sdk = getSdkContext();
  const prompt = $derived(page.url.searchParams.get("prompt") ?? "hello");

  let email = $state("");
  let password = $state("");
  let isSubmitting = $state(false);
  let error = $state<string | null>(null);

  async function handleSubmit() {
    if (isSubmitting) return;
    error = null;
    isSubmitting = true;

    try {
      const session = await sdk.auth.login(email.trim(), password);
      setWebAccessToken(session.access_token);
      await goto("/");
    } catch {
      error = "We could not sign you in. Check your email and password, then try again.";
    } finally {
      isSubmitting = false;
    }
  }
</script>

<svelte:head>
  <title>Account · Epifly</title>
</svelte:head>

<AuthOnboardingPanel
  {prompt}
  {email}
  {password}
  {error}
  {isSubmitting}
  onEmailChange={(value) => (email = value)}
  onPasswordChange={(value) => (password = value)}
  onSubmit={handleSubmit}
/>
