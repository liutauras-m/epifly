export type UiMessage = {
  id: string;
  role: "user" | "assistant";
  content: string;
  /** True while the assistant message is still receiving stream deltas. */
  pending?: boolean;
};
