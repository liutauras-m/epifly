<script lang="ts">
  import { page } from "$app/state";
  import { getSdkContext, createChatStore, loadThreadMessages, getWorkspaceNodeContext, getActiveThreadNodeContext, getWorkspaceActionsContext, uploadUiAttachment } from "@epifly/features";
  import { AppSafeArea, ChatBreadcrumb, ChatComposer, ChatMessageList, ChatEmptyState, WorkspaceFilingSuggestion } from "@epifly/ui";

  const sdk = getSdkContext();
  const chat = createChatStore(sdk);
  const wsNode = getWorkspaceNodeContext();
  const threadNode = getActiveThreadNodeContext();
  const wsActions = getWorkspaceActionsContext();

  /** Step 3.4 — reactive filing hint from the shell (null = no suggestion). */
  const filingHint = $derived(wsActions?.getFilingHint() ?? null);

  let isLoadingHistory = $state(true);
  let fileInputEl = $state<HTMLInputElement | null>(null);
  let pendingAttachmentIds = $state<string[]>([]);
  let isUploading = $state(false);

  /**
   * Step 6.2 — user can detach the folder context so the next send goes
   * without a workspaceNodeId. Reset whenever the thread changes.
   */
  let contextDetached = $state(false);

  const threadId = $derived(page.params.threadId);

  // Step 6.1 — the folder that contains this thread (parent folder node_id).
  // Falls back to the sidebar-selected node, then null (no context).
  const activeContextNodeId = $derived(
    contextDetached ? null : (threadNode.folderNodeId ?? wsNode.current)
  );

  // Reload history whenever threadId changes (navigating between threads).
  $effect(() => {
    const id = threadId;
    isLoadingHistory = true;
    contextDetached = false; // reset detach on navigation
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
    // Step 6.1 — pass ambient folder context to the agent.
    await chat.send(msg, activeContextNodeId, ids);
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
  <!-- Breadcrumb + context indicator (Steps 1.4 / 1.5 / 6.2) -->
  {#if threadNode.virtualPath}
    <div class="flex shrink-0 items-center gap-2 border-b border-border/40 px-6 py-1.5">
      <ChatBreadcrumb
        virtualPath={threadNode.virtualPath}
        onCrumbClick={(path) => wsActions?.selectNodeByPath(path)}
      />

      <!-- Step 6.2 — live context disclosure chip with detach/re-attach -->
      {#if !contextDetached && activeContextNodeId}
        <button
          type="button"
          class="ml-auto flex shrink-0 items-center gap-1 rounded-full bg-muted/60 px-2 py-0.5 text-[0.65rem] text-muted-foreground/70 hover:bg-muted hover:text-muted-foreground"
          title="Using context from {threadNode.placeName ?? 'workspace'} — click to detach"
          onclick={() => (contextDetached = true)}
          aria-label="Context: {threadNode.placeName ?? 'Workspace'} — click to detach"
        >
          <span>Using: {threadNode.placeName ?? "Workspace"}</span>
          <span aria-hidden="true" class="opacity-60">×</span>
        </button>
      {:else}
        <button
          type="button"
          class="ml-auto flex shrink-0 items-center gap-1 rounded-full bg-muted/30 px-2 py-0.5 text-[0.65rem] text-muted-foreground/40 hover:bg-muted/60 hover:text-muted-foreground/70"
          title="No folder context — click to re-attach"
          onclick={() => (contextDetached = false)}
          aria-label="No context — click to re-attach"
        >
          <span>No context</span>
          <span aria-hidden="true" class="opacity-60">+</span>
        </button>
      {/if}
    </div>
  {/if}

  <!-- Step 3.4 — filing suggestion chip (only for unsorted threads) -->
  {#if filingHint}
    <div class="mx-auto w-full max-w-3xl shrink-0 px-6 py-2">
      <WorkspaceFilingSuggestion
        suggestedPath={filingHint.virtualPath}
        targetNodeId={filingHint.id}
        onConfirm={(nodeId, path) => wsActions?.confirmFiling(nodeId, path)}
        onIgnore={() => wsActions?.dismissFilingHint()}
      />
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
