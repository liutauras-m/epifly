<script lang="ts">
  import { cn } from "../../utils/cn.js";
  import type { Snippet } from "svelte";

  type Props = {
    class?: string;
    children?: Snippet;
  };

  let { class: className, children }: Props = $props();
</script>

<main class={cn("flex flex-1 flex-col overflow-hidden", className)}>
  {@render children?.()}
</main>
