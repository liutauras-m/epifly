import type { ConusSdk, ChatStreamDelta } from "@conusai/sdk";
import type { UiMessage, UiTextMessage, UiStreamEvent } from "./chat.types.js";

export function createChatStore(sdk: ConusSdk) {
  let messages = $state<UiMessage[]>([]);
  let isStreaming = $state(false);
  let threadId = $state<string | null>(null);
  let error = $state<string | null>(null);
  let abortController = $state<AbortController | null>(null);

  async function send(
    message: string,
    workspaceNodeId?: string | null,
    attachmentIds?: string[]
  ) {
    const trimmed = message.trim();
    if (!trimmed || isStreaming) return;

    error = null;
    isStreaming = true;

    const controller = new AbortController();
    abortController = controller;

    messages.push({
      id: crypto.randomUUID(),
      role: "user",
      content: trimmed
    } satisfies UiTextMessage);

    const assistantMessageId = crypto.randomUUID();
    messages.push({
      id: assistantMessageId,
      role: "assistant",
      content: "",
      pending: true
    } satisfies UiTextMessage);

    try {
      for await (const delta of sdk.chat.stream({
        message: trimmed,
        threadId,
        workspaceNodeId,
        attachmentIds: attachmentIds?.length ? attachmentIds : undefined,
        signal: controller.signal
      })) {
        applyDelta(delta, assistantMessageId);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      setAssistantPending(assistantMessageId, false);
    } finally {
      isStreaming = false;
      abortController = null;
      setAssistantPending(assistantMessageId, false);
    }
  }

  function findAssistantMessage(assistantMessageId: string) {
    return messages.find(
      (msg): msg is UiTextMessage => msg.role === "assistant" && msg.id === assistantMessageId
    );
  }

  function setAssistantPending(assistantMessageId: string, pending: boolean) {
    const assistantMessage = findAssistantMessage(assistantMessageId);
    if (assistantMessage) assistantMessage.pending = pending;
  }

  function insertBeforeAssistant(event: UiStreamEvent, assistantMessageId: string) {
    const assistantIdx = messages.findIndex((msg) => msg.id === assistantMessageId);
    if (assistantIdx !== -1) {
      messages.splice(assistantIdx, 0, event);
    } else {
      messages.push(event);
    }
  }

  function applyDelta(delta: ChatStreamDelta, assistantMessageId: string) {
    switch (delta.kind) {
      case "text": {
        const assistantMessage = findAssistantMessage(assistantMessageId);
        if (assistantMessage) assistantMessage.content += delta.content;
        break;
      }

      case "thread_id":
        threadId = delta.id;
        break;

      case "tool_start": {
        const event: UiStreamEvent = {
          id: crypto.randomUUID(),
          role: "event",
          kind: "tool_start",
          toolName: delta.name,
          toolUseId: delta.id
        };
        insertBeforeAssistant(event, assistantMessageId);
        break;
      }

      case "tool_result": {
        // Find the matching tool_start event and update it in-place.
        const existing = messages.find(
          (m): m is UiStreamEvent =>
            m.role === "event" &&
            m.kind === "tool_start" &&
            m.toolUseId === delta.tool_use_id
        );
        if (existing) {
          existing.kind = "tool_result";
          existing.result = delta.result;
          existing.error = delta.error;
        } else {
          const event: UiStreamEvent = {
            id: crypto.randomUUID(),
            role: "event",
            kind: "tool_result",
            toolUseId: delta.tool_use_id,
            result: delta.result,
            error: delta.error
          };
          insertBeforeAssistant(event, assistantMessageId);
        }
        break;
      }

      case "routing_meta": {
        const event: UiStreamEvent = {
          id: crypto.randomUUID(),
          role: "event",
          kind: "routing_meta",
          capabilities: delta.selected_capabilities
        };
        insertBeforeAssistant(event, assistantMessageId);
        break;
      }

      case "resource_invalidated": {
        const event: UiStreamEvent = {
          id: crypto.randomUUID(),
          role: "event",
          kind: "resource_invalidated",
          resource: delta.resource
        };
        insertBeforeAssistant(event, assistantMessageId);
        break;
      }

      case "done":
        setAssistantPending(assistantMessageId, false);
        break;
    }
  }

  function stop() {
    abortController?.abort();
  }

  function reset() {
    messages = [];
    threadId = null;
    error = null;
    isStreaming = false;
    abortController = null;
  }

  /** Populate the store with previously loaded messages (e.g. when opening an existing thread). */
  function init(initialMessages: UiMessage[], initialThreadId?: string | null) {
    messages = [...initialMessages];
    threadId = initialThreadId ?? null;
    error = null;
    isStreaming = false;
    abortController = null;
  }

  return {
    get messages() { return messages; },
    get isStreaming() { return isStreaming; },
    get threadId() { return threadId; },
    get error() { return error; },
    send,
    stop,
    reset,
    init
  };
}
