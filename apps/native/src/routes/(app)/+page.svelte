<script lang="ts">
  import { goto } from "$app/navigation";
  import { getSdkContext, createChatStore } from "@epifly/features";
  import { AppSafeArea, ChatComposer, ChatEmptyState, ChatMessageList } from "@epifly/ui";

  const sdk = getSdkContext();
  const chat = createChatStore(sdk);

  let hasNavigated = $state(false);

  $effect(() => {
    if (!hasNavigated && !chat.isStreaming && chat.threadId && chat.messages.length > 0) {
      hasNavigated = true;
      goto(`/chat/${chat.threadId}`, { replaceState: true });
    }
  });
</script>

<svelte:head>
  <title>Epifly Chat</title>
</svelte:head>

<AppSafeArea class="flex min-h-0 flex-1 flex-col">
  {#if chat.messages.length === 0}
    <section class="flex min-h-0 flex-1 items-center justify-center px-6 py-16" aria-label="Start a conversation">
      <ChatEmptyState
        title="How can Epifly help?"
        description="Ask anything or start with a workspace file."
      />
    </section>
  {:else}
    <ChatMessageList messages={chat.messages} class="flex-1" />
  {/if}

  {#if chat.error}
    <p role="alert" class="px-6 py-2 text-sm text-destructive">{chat.error}</p>
  {/if}

  <ChatComposer
    isStreaming={chat.isStreaming}
    placeholder="How can Epifly help?"
    onSubmit={(msg) => chat.send(msg)}
    onStop={() => chat.stop()}
  />
</AppSafeArea>
