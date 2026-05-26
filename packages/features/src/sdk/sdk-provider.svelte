<script lang="ts">
  import { createConusSdk, type TokenProvider } from "@conusai/sdk";
  import { setSdkContext } from "./sdk-context.svelte.js";
  import { untrack } from "svelte";
  import type { Snippet } from "svelte";

  type Props = {
    baseUrl: string;
    tokenProvider: TokenProvider;
    children?: Snippet;
  };

  let { baseUrl, tokenProvider, children }: Props = $props();

  const sdk = untrack(() => createConusSdk({
    baseUrl,
    tokenProvider,
    fetch: globalThis.fetch.bind(globalThis)
  }));

  setSdkContext(sdk);
</script>

{@render children?.()}
