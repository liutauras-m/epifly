<script lang="ts">
  import "../app.css";
  import { SdkProvider, createWebTokenProvider } from "@epifly/features";
  import type { Snippet } from "svelte";

  type Props = { children?: Snippet };
  let { children }: Props = $props();

  // BFF proxy handles auth injection; token provider returns null
  const tokenProvider = createWebTokenProvider();
</script>

<SdkProvider baseUrl="/api" {tokenProvider}>
  {@render children?.()}
</SdkProvider>
