export type StreamEventKind =
  | "tool_start"
  | "tool_result"
  | "routing_meta"
  | "resource_invalidated";

/** A user or assistant text message in the chat thread. */
export type UiTextMessage = {
  id: string;
  role: "user" | "assistant";
  content: string;
  /** True while the assistant message is still receiving stream deltas. */
  pending?: boolean;
};

/**
 * A structured stream event surfaced inline in the chat thread.
 * Rendered as ToolEventRow / RoutingMetaRow in the UI.
 */
export type UiStreamEvent = {
  id: string;
  role: "event";
  kind: StreamEventKind;
  /** Tool name — present on tool_start and tool_result. */
  toolName?: string;
  /** Correlates tool_result back to the originating tool_start. */
  toolUseId?: string;
  /** Raw result text — present on tool_result. */
  result?: string;
  /** Error description when tool_result failed. */
  error?: string;
  /** Selected capabilities — present on routing_meta. */
  capabilities?: string[];
  /** Invalidated resource key — present on resource_invalidated. */
  resource?: string;
};

export type UiMessage = UiTextMessage | UiStreamEvent;
