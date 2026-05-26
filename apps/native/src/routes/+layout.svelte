<script lang="ts">
  import "../app.css";
  import { env } from "$env/dynamic/public";
  import { SdkProvider, createNativeTokenProvider } from "@epifly/features";
  import type { Snippet } from "svelte";

  type Props = { children?: Snippet };
  let { children }: Props = $props();

  const tokenProvider = createNativeTokenProvider();
  const baseUrl = env.PUBLIC_API_URL || "http://localhost:3001";
</script>

<SdkProvider {baseUrl} {tokenProvider}>
  {@render children?.()}
</SdkProvider>
