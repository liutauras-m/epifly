<script lang="ts">
  import "../app.css";
  import { env } from "$env/dynamic/public";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { SdkProvider } from "@epifly/features";
  import { createNativeTokenProvider } from "$lib/native/token-provider.js";
  import { applySafeAreaInsets } from "$lib/native/safe-area.js";
  import { auth } from "$lib/stores/auth.svelte.js";
  import type { Snippet } from "svelte";

  type Props = { children?: Snippet };
  let { children }: Props = $props();

  // Uses OS keychain via Rust Tauri command — JS never holds the refresh token
  const tokenProvider = createNativeTokenProvider();
  const baseUrl = env.PUBLIC_API_URL || "http://localhost:8080";

  onMount(() => {
    const style = getComputedStyle(document.documentElement);
    const rawBottom = style.getPropertyValue("--safe-bottom").trim();
    const rawTop = style.getPropertyValue("--safe-top").trim();
    const bottomPx = parseFloat(rawBottom) || 0;
    const topPx = parseFloat(rawTop) || 0;

    if (bottomPx === 0) applySafeAreaInsets({ bottom: 34 });
    if (topPx === 0) applySafeAreaInsets({ top: 54 });
  });

  const LOGIN_PATH = "/login";
  const APP_PATHS = ["/", "/chat", "/workspaces", "/settings"];

  // Route guard: redirect unauthenticated users to /login,
  // redirect authenticated users away from /login.
  $effect(() => {
    const isLoginPage = page.url.pathname === LOGIN_PATH;
    if (auth.status === "unauthenticated" && !isLoginPage) {
      void goto(LOGIN_PATH, { replaceState: true });
    } else if (auth.status === "authenticated" && isLoginPage) {
      void goto("/", { replaceState: true });
    }
  });
</script>

<SdkProvider {baseUrl} {tokenProvider}>
  {#if auth.status === "checking"}
    <!-- Splash / loading state while we check keychain -->
    <div
      class="flex min-h-screen flex-col items-center justify-center bg-background"
      role="status"
      aria-label="Loading"
    >
      <div
        class="flex h-14 w-14 items-center justify-center rounded-[20px]"
        style="background: linear-gradient(135deg, #ff7a1a 0%, #ff6200 100%)"
        aria-hidden="true"
      >
        <svg width="28" height="28" viewBox="0 0 32 32" fill="none" aria-hidden="true">
          <path d="M8 24 L16 8 L24 24" stroke="white" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
          <path d="M11 19 L21 19" stroke="white" stroke-width="2.5" stroke-linecap="round"/>
        </svg>
      </div>
    </div>
  {:else}
    {@render children?.()}
  {/if}
</SdkProvider>
