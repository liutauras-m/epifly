<script lang="ts">
  import { cn } from "../../utils/cn.js";

  type Props = {
    capability?: string;
    class?: string;
  };

  let { capability, class: className }: Props = $props();
</script>

<div
  class={cn(
    "flex items-center gap-2 py-1 text-xs text-muted-foreground",
    className
  )}
>
  <span
    class="h-1.5 w-1.5 rounded-full bg-primary/40"
    aria-hidden="true"
  ></span>
  {#if capability}
    <span>Routing to <em class="not-italic font-medium">{capability}</em></span>
  {:else}
    <span>Routing request…</span>
  {/if}
</div>
