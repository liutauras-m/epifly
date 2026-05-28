<script lang="ts">
  import { page } from "$app/state";

  const MESSAGES: Record<string, string> = {
    exchange_failed: "Sign-in failed — the authorization code could not be exchanged. Please try again.",
    missing_tokens: "Sign-in failed — the identity provider did not return tokens. Please try again.",
    missing_claims: "Sign-in failed — required identity claims are missing. Contact support if this persists.",
    missing_org_claim: "Your account is not linked to an organisation. Ask your admin to add you.",
    missing_sub: "Sign-in failed — user identity is missing. Contact support.",
    transaction_already_consumed: "This sign-in link has already been used. Please start a new sign-in.",
    state_mismatch: "Sign-in failed — security check did not pass. Please try again.",
    expired_session: "Your session expired. Please sign in again.",
    email_not_verified: "Please verify your email address before signing in.",
    tenant_not_provisioned: "Your organisation has not been provisioned yet. Contact your admin.",
    user_cancelled: "Sign-in was cancelled.",
  };

  let reason = $derived(page.url.searchParams.get("reason") ?? "unknown");
  let message = $derived(
    MESSAGES[reason] ?? "An unexpected error occurred during sign-in. Please try again."
  );
</script>

<svelte:head>
  <title>Sign-in error — Epifly</title>
</svelte:head>

<div class="flex min-h-svh items-center justify-center p-6">
  <div class="w-full max-w-md space-y-4 text-center">
    <p class="text-sm text-muted-foreground">{message}</p>
    <a
      href="/auth/login"
      class="inline-flex items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
    >
      Try again
    </a>
  </div>
</div>
