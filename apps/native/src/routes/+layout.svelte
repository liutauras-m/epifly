<script lang="ts">
  import "../app.css";
  import { env } from "$env/dynamic/public";
  import { onMount } from "svelte";
  import { SdkProvider } from "@epifly/features";
  import { createNativeTokenProvider } from "$lib/native/token-provider.js";
  import { applySafeAreaInsets } from "$lib/native/safe-area.js";
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
</script>

<SdkProvider {baseUrl} {tokenProvider}>
  {@render children?.()}
</SdkProvider>
