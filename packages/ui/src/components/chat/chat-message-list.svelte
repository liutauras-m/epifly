<script lang="ts">
  import ChatMessage from "./chat-message.svelte";
  import { cn } from "../../utils/cn.js";

  type Message = {
    id: string;
    role: "user" | "assistant";
    content: string;
    pending?: boolean;
  };

  type Props = {
    messages: Message[];
    class?: string;
  };

  let { messages, class: className }: Props = $props();

  let listEl = $state<HTMLDivElement | null>(null);

  $effect(() => {
    // Scroll to bottom on new messages
    if (listEl && messages.length > 0) {
      listEl.scrollTop = listEl.scrollHeight;
    }
  });
</script>

<div
  bind:this={listEl}
  class={cn("flex flex-1 flex-col gap-4 overflow-y-auto p-4 sm:gap-5 sm:p-6", className)}
>
  {#each messages as msg (msg.id)}
    <ChatMessage role={msg.role} content={msg.content} pending={msg.pending} />
  {/each}
</div>
