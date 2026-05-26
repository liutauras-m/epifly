<script lang="ts">
  import { cn } from "../../utils/cn.js";
  import ChatMessage from "./chat-message.svelte";
  import ToolEventRow from "./tool-event-row.svelte";
  import RoutingMetaRow from "./routing-meta-row.svelte";

  // Mirror of UiMessage from @epifly/features — defined locally so packages/ui
  // stays independent of packages/features (structural typing ensures compatibility).
  type TextMessage = {
    id: string;
    role: "user" | "assistant";
    content: string;
    pending?: boolean;
  };

  type StreamEvent = {
    id: string;
    role: "event";
    kind: "tool_start" | "tool_result" | "routing_meta" | "resource_invalidated";
    toolName?: string;
    capabilities?: string[];
    resource?: string;
    error?: string;
  };

  type Message = TextMessage | StreamEvent;

  type Props = {
    messages: Message[];
    class?: string;
  };

  let { messages, class: className }: Props = $props();

  let listEl = $state<HTMLDivElement | null>(null);

  const scrollSignature = $derived(
    messages
      .map((msg) => {
        if (msg.role === "event") {
          return `${msg.id}:${msg.kind}:${msg.error ?? ""}`;
        }

        return `${msg.id}:${msg.content}:${msg.pending ? "pending" : "done"}`;
      })
      .join("|")
  );

  $effect(() => {
    scrollSignature;
    if (listEl && messages.length > 0) {
      requestAnimationFrame(() => {
        if (listEl) listEl.scrollTop = listEl.scrollHeight;
      });
    }
  });
</script>

<div
  bind:this={listEl}
  class={cn(
    "min-h-0 flex-1 basis-0 scroll-pb-32 overflow-y-auto overscroll-contain scroll-smooth px-4 pb-16 pt-2 sm:px-6",
    className
  )}
>
  <div class="mx-auto flex w-full max-w-3xl flex-col gap-1 sm:gap-1.5">
    {#each messages as msg (msg.id)}
      {#if msg.role === "event"}
        {#if msg.kind === "routing_meta"}
          <RoutingMetaRow capability={msg.capabilities?.[0]} class="my-1" />
        {:else}
          <ToolEventRow
            kind={msg.kind}
            label={msg.toolName}
            class={cn("my-1", msg.kind === "tool_result" && msg.error && "text-destructive")}
          />
        {/if}
      {:else}
        <ChatMessage
          role={msg.role}
          content={msg.content}
          pending={msg.pending}
          class="mb-3"
        />
      {/if}
    {/each}
  </div>
</div>
