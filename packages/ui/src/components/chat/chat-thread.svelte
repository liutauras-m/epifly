<script lang="ts">
  import ChatMessageList from "./chat-message-list.svelte";
  import ChatEmptyState from "./chat-empty-state.svelte";

  type Message = {
    id: string;
    role: "user" | "assistant";
    content: string;
    pending?: boolean;
  };

  type Props = {
    messages: Message[];
  };

  let { messages }: Props = $props();
</script>

<div class="flex flex-1 flex-col overflow-hidden">
  {#if messages.length === 0}
    <ChatEmptyState />
  {:else}
    <ChatMessageList {messages} />
  {/if}
</div>
