<script lang="ts">
  import "../app.css";
  import { env } from "$env/dynamic/public";
  import { onMount } from "svelte";
  import { SdkProvider, createNativeTokenProvider } from "@epifly/features";
  import { applySafeAreaInsets } from "$lib/native/safe-area.js";
  import type { Snippet } from "svelte";

  type Props = { children?: Snippet };
  let { children }: Props = $props();

  const tokenProvider = createNativeTokenProvider();
  const baseUrl = env.PUBLIC_API_URL || "http://localhost:8080";

  onMount(() => {
    // env(safe-area-inset-*) is unreliable in WKWebView without explicit native bridging.
    // Read the computed CSS values after a tick to let the browser parse them; if they come
    // back empty/0, fall back to the known iOS hardware safe-area values.
    const style = getComputedStyle(document.documentElement);
    const rawBottom = style.getPropertyValue("--safe-bottom").trim();
    const rawTop = style.getPropertyValue("--safe-top").trim();
    const bottomPx = parseFloat(rawBottom) || 0;
    const topPx = parseFloat(rawTop) || 0;

    // iPhone notch top ~54pt, home indicator bottom ~34pt — only override when env() gave 0.
    if (bottomPx === 0) applySafeAreaInsets({ bottom: 34 });
    if (topPx === 0) applySafeAreaInsets({ top: 54 });
  });
</script>

<SdkProvider {baseUrl} {tokenProvider}>
  {@render children?.()}
</SdkProvider>
