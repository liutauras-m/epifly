<script lang="ts">
  import { goto } from "$app/navigation";
  import { getSdkContext, createChatStore, getWorkspaceNodeContext, uploadUiAttachment } from "@epifly/features";
  import { AppSafeArea, ChatComposer, ChatEmptyState, ChatMessageList } from "@epifly/ui";

  const sdk = getSdkContext();
  const chat = createChatStore(sdk);
  const wsNode = getWorkspaceNodeContext();

  let hasNavigated = $state(false);
  let fileInputEl = $state<HTMLInputElement | null>(null);
  let pendingAttachmentIds = $state<string[]>([]);
  let isUploading = $state(false);

  // After streaming completes and a threadId is assigned, navigate to the thread page.
  $effect(() => {
    if (!hasNavigated && !chat.isStreaming && chat.threadId && chat.messages.length > 0) {
      hasNavigated = true;
      goto(`/chat/${chat.threadId}`, { replaceState: true });
    }
  });

  async function handleSubmit(msg: string) {
    const ids = pendingAttachmentIds.length ? [...pendingAttachmentIds] : undefined;
    pendingAttachmentIds = [];
    await chat.send(msg, wsNode.current, ids);
  }

  function handleAttach() {
    fileInputEl?.click();
  }

  async function handleFileChange(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    isUploading = true;
    const result = await uploadUiAttachment(sdk, file);
    isUploading = false;
    if (result.data) {
      pendingAttachmentIds = [...pendingAttachmentIds, result.data.id];
    }
    input.value = "";
  }
</script>

<svelte:head>
  <title>Epifly Chat</title>
</svelte:head>

<!-- Hidden file input for attachment uploads -->
<input
  bind:this={fileInputEl}
  type="file"
  class="sr-only"
  accept="image/*,.pdf,.txt,.md,.csv,.docx,.xlsx"
  onchange={handleFileChange}
  aria-hidden="true"
  tabindex="-1"
/>

<AppSafeArea class="flex h-full min-h-0 flex-1 flex-col pt-[calc(var(--sidebar-toggle-offset)+2.75rem)]">
  {#if chat.messages.length === 0}
    <section class="flex min-h-0 flex-1 items-start justify-center px-6 pb-16 pt-12 sm:pt-20" aria-label="Start a conversation">
      <ChatEmptyState
        title="How can Epifly help?"
        description="Ask anything or start with a workspace file."
      />
    </section>
  {:else}
    <ChatMessageList messages={chat.messages} class="flex-1" />
  {/if}

  {#if chat.error}
    <p role="alert" class="mx-auto w-full max-w-3xl px-6 py-2 text-sm text-destructive">{chat.error}</p>
  {/if}

  {#if pendingAttachmentIds.length > 0}
    <p class="mx-auto w-full max-w-3xl px-6 py-1 text-xs text-muted-foreground">
      {pendingAttachmentIds.length} file{pendingAttachmentIds.length > 1 ? "s" : ""} attached
    </p>
  {/if}

  <ChatComposer
    isStreaming={chat.isStreaming || isUploading}
    placeholder="How can Epifly help?"
    onSubmit={handleSubmit}
    onStop={() => chat.stop()}
    onAttach={handleAttach}
  />
</AppSafeArea>
