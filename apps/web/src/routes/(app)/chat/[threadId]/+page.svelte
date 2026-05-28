<script lang="ts">
  import { page } from "$app/state";
  import { getSdkContext, createChatStore, loadThreadMessages, getWorkspaceNodeContext, getActiveThreadNodeContext, uploadUiAttachment } from "@epifly/features";
  import { AppSafeArea, ChatBreadcrumb, ChatComposer, ChatMessageList, ChatEmptyState } from "@epifly/ui";

  const sdk = getSdkContext();
  const chat = createChatStore(sdk);
  const wsNode = getWorkspaceNodeContext();
  const threadNode = getActiveThreadNodeContext();

  let isLoadingHistory = $state(true);
  let fileInputEl = $state<HTMLInputElement | null>(null);
  let pendingAttachmentIds = $state<string[]>([]);
  let isUploading = $state(false);

  const threadId = $derived(page.params.threadId);

  // Reload history whenever threadId changes (navigating between threads).
  $effect(() => {
    const id = threadId;
    isLoadingHistory = true;
    chat.reset();
    if (!id) {
      isLoadingHistory = false;
      return;
    }

    let cancelled = false;
    loadThreadMessages(sdk, id).then((result) => {
      if (cancelled) return;
      if (result.data) {
        chat.init(result.data, id);
      }
      isLoadingHistory = false;
    });

    return () => {
      cancelled = true;
    };
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
  <title>Chat · Epifly</title>
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

<AppSafeArea class="flex h-full min-h-0 flex-1 flex-col pt-[calc(var(--safe-top)+var(--sidebar-toggle-offset)+2.75rem)]">
  <!-- Breadcrumb + context indicator (Steps 1.4 / 1.5) -->
  {#if threadNode.virtualPath}
    <div class="flex shrink-0 items-center gap-2 border-b border-border/40 px-6 py-1.5">
      <ChatBreadcrumb virtualPath={threadNode.virtualPath} />
      <span class="ml-auto shrink-0 rounded-full bg-muted/60 px-2 py-0.5 text-[0.65rem] text-muted-foreground/70">
        Context: {threadNode.placeName ?? "Workspace"}
      </span>
    </div>
  {/if}

  {#if isLoadingHistory}
    <div class="flex flex-1 items-center justify-center" aria-label="Loading conversation">
      <span class="text-sm text-muted-foreground">Loading…</span>
    </div>
  {:else if chat.messages.length === 0}
    <section class="flex min-h-0 flex-1 items-start justify-center px-6 pb-16 pt-12 sm:pt-20">
      <ChatEmptyState
        title="Continue the conversation"
        description="This thread has no messages yet."
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
    placeholder="Reply…"
    onSubmit={handleSubmit}
    onStop={() => chat.stop()}
    onAttach={handleAttach}
  />
</AppSafeArea>
