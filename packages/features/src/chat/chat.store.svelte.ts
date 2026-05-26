import type { ConusSdk, ChatStreamDelta } from "@conusai/sdk";
import type { UiMessage } from "./chat.types.js";

export function createChatStore(sdk: ConusSdk) {
  let messages = $state<UiMessage[]>([]);
  let isStreaming = $state(false);
  let threadId = $state<string | null>(null);
  let error = $state<string | null>(null);
  let abortController = $state<AbortController | null>(null);

  async function send(message: string, workspaceNodeId?: string | null) {
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
    });

    const assistantMessage: UiMessage = {
      id: crypto.randomUUID(),
      role: "assistant",
      content: "",
      pending: true
    };

    messages.push(assistantMessage);

    try {
      for await (const delta of sdk.chat.stream({
        message: trimmed,
        threadId,
        workspaceNodeId,
        signal: controller.signal
      })) {
        applyDelta(delta, assistantMessage);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      assistantMessage.pending = false;
    } finally {
      isStreaming = false;
      abortController = null;
      assistantMessage.pending = false;
    }
  }

  function applyDelta(delta: ChatStreamDelta, assistantMessage: UiMessage) {
    switch (delta.kind) {
      case "text":
        assistantMessage.content += delta.content;
        break;

      case "thread_id":
        threadId = delta.id;
        break;

      case "tool_start":
      case "tool_result":
      case "routing_meta":
      case "resource_invalidated":
        // TODO: surface as structured stream events in chat-stream-status.svelte
        break;

      case "done":
        assistantMessage.pending = false;
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

  return {
    get messages() { return messages; },
    get isStreaming() { return isStreaming; },
    get threadId() { return threadId; },
    get error() { return error; },
    send,
    stop,
    reset
  };
}
