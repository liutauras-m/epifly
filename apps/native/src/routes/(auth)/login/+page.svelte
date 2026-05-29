<script lang="ts">
  import { auth } from "$lib/stores/auth.svelte.js";
</script>

<svelte:head>
  <title>Sign in · Epifly</title>
</svelte:head>

<div class="flex min-h-screen flex-col items-center justify-between bg-background px-8 pb-[max(var(--safe-bottom),2rem)] pt-[max(var(--safe-top),3rem)]">
  <!-- Top spacer -->
  <div class="flex-1" aria-hidden="true"></div>

  <!-- Hero -->
  <div class="flex w-full max-w-xs flex-col items-center gap-8">
    <!-- Brand mark -->
    <div class="flex flex-col items-center gap-4">
      <div
        class="flex h-16 w-16 items-center justify-center rounded-[22px] shadow-[0_8px_24px_color-mix(in_oklch,var(--epifly-logo-orange)_28%,transparent)]"
        style="background: linear-gradient(135deg, #ff7a1a 0%, #ff6200 100%)"
        aria-hidden="true"
      >
        <svg width="32" height="32" viewBox="0 0 32 32" fill="none" aria-hidden="true">
          <path d="M8 24 L16 8 L24 24" stroke="white" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
          <path d="M11 19 L21 19" stroke="white" stroke-width="2.5" stroke-linecap="round"/>
        </svg>
      </div>
      <div class="text-center">
        <h1 class="text-2xl font-semibold tracking-tight text-foreground">Welcome to Epifly</h1>
        <p class="mt-1.5 text-sm text-muted-foreground">Your AI workspace, everywhere.</p>
      </div>
    </div>

    <!-- Sign-in button -->
    <button
      onclick={() => auth.login()}
      disabled={auth.status === "login_pending"}
      data-testid="auth-login-cta"
      class="relative flex w-full items-center justify-center gap-2.5 rounded-2xl bg-primary px-6 py-4 text-base font-semibold text-primary-foreground shadow-md transition-all duration-150 active:scale-[0.97] disabled:opacity-60"
      aria-label="Sign in with Epifly"
    >
      {#if auth.status === "login_pending"}
        <span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-primary-foreground border-t-transparent" aria-hidden="true"></span>
        Opening browser…
      {:else}
        <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
          <rect x="2" y="2" width="6" height="6" rx="1.5" fill="currentColor" opacity=".8"/>
          <rect x="10" y="2" width="6" height="6" rx="1.5" fill="currentColor"/>
          <rect x="2" y="10" width="6" height="6" rx="1.5" fill="currentColor"/>
          <rect x="10" y="10" width="6" height="6" rx="1.5" fill="currentColor" opacity=".8"/>
        </svg>
        Continue with Epifly
      {/if}
    </button>

    <!-- Error message -->
    {#if auth.error}
      <p role="alert" class="text-center text-sm text-destructive">{auth.error}</p>
    {/if}

    <!-- Legal note -->
    <p class="text-center text-xs leading-relaxed text-muted-foreground/70">
      By continuing you agree to the Epifly<br />
      <a href="https://epifly.app/terms" class="underline underline-offset-2">Terms of Service</a>
      &nbsp;and&nbsp;
      <a href="https://epifly.app/privacy" class="underline underline-offset-2">Privacy Policy</a>.
    </p>
  </div>

  <!-- Bottom spacer (equal to top) -->
  <div class="flex-1" aria-hidden="true"></div>
</div>
