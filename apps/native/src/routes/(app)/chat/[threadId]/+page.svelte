<script lang="ts">
  import { page } from "$app/state";
  import { getSdkContext, createChatStore, loadThreadMessages } from "@epifly/features";
  import { AppSafeArea, ChatComposer, ChatMessageList, ChatEmptyState } from "@epifly/ui";

  const sdk = getSdkContext();
  const chat = createChatStore(sdk);

  let isLoadingHistory = $state(true);

  const threadId = $derived(page.params.threadId);

  $effect(() => {
    const id = threadId;
    isLoadingHistory = true;
    chat.reset();
    loadThreadMessages(sdk, id).then((result) => {
      if (result.data) {
        chat.init(result.data, id);
      }
      isLoadingHistory = false;
    });
  });
</script>

<svelte:head>
  <title>Chat · Epifly</title>
</svelte:head>

<AppSafeArea class="flex min-h-0 flex-1 flex-col">
  {#if isLoadingHistory}
    <div class="flex flex-1 items-center justify-center" aria-label="Loading conversation">
      <span class="text-sm text-muted-foreground">Loading…</span>
    </div>
  {:else if chat.messages.length === 0}
    <section class="flex min-h-0 flex-1 items-center justify-center px-6 py-16">
      <ChatEmptyState
        title="Continue the conversation"
        description="This thread has no messages yet."
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
    placeholder="Reply…"
    onSubmit={(msg) => chat.send(msg, null)}
    onStop={() => chat.stop()}
  />
</AppSafeArea>
