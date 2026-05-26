<script lang="ts">
  import MessageCircleIcon from "@lucide/svelte/icons/message-circle";
  import { cn } from "../../utils/cn.js";
  import * as Empty from "../ui/empty/index.js";
  import type { Snippet } from "svelte";

  type Props = {
    class?: string;
    eyebrow?: string;
    title?: string;
    description?: string;
    actions?: Snippet;
  };

  let {
    class: className,
    eyebrow,
    title = "Start a conversation",
    description = "Send a message below to get started.",
    actions
  }: Props = $props();
</script>

<Empty.Root
  class={cn(
    "motion-enter border-0 bg-transparent p-8",
    className
  )}
>
  <Empty.Header class="max-w-md gap-5">
    <Empty.Media variant="icon" class="size-12 rounded-full text-muted-foreground [&_svg:not([class*='size-'])]:size-6">
      <MessageCircleIcon strokeWidth={1.5} aria-hidden="true" />
    </Empty.Media>
    {#if eyebrow}
      <p class="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">
        {eyebrow}
      </p>
    {/if}
    <Empty.Title
      role="heading"
      aria-level={1}
      class="text-balance text-2xl font-semibold tracking-[-0.04em] text-foreground sm:text-3xl"
    >
      {title}
    </Empty.Title>
    <Empty.Description class="text-pretty text-sm leading-6 text-muted-foreground">
      {description}
    </Empty.Description>
  </Empty.Header>
  {#if actions}
    <Empty.Content class="flex-row flex-wrap justify-center gap-2">
      {@render actions()}
    </Empty.Content>
  {/if}
</Empty.Root>
