<script lang="ts">
  import { cn } from "../../utils/cn.js";
  import type { Snippet } from "svelte";

  type Props = {
    class?: string;
    children?: Snippet;
  };

  let { class: className, children }: Props = $props();
</script>

<!-- Applies safe-area insets so content is never obscured by notches or home bars. -->
<div
  class={cn("pb-[var(--safe-bottom)] pt-[var(--safe-top)]", className)}
>
  {@render children?.()}
</div>
