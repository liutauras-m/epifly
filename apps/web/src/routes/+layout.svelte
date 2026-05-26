<script lang="ts">
  import "../app.css";
  import { SdkProvider, createWebTokenProvider } from "@epifly/features";
  import { PUBLIC_API_URL } from "$env/static/public";
  import type { Snippet } from "svelte";

  type Props = { children?: Snippet };
  let { children }: Props = $props();

  const tokenProvider = createWebTokenProvider();
</script>

<SdkProvider baseUrl={PUBLIC_API_URL} {tokenProvider}>
  {@render children?.()}
</SdkProvider>
