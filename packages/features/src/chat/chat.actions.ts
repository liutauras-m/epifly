import type { ConusSdk } from "@conusai/sdk";
import type { ApiResult } from "@conusai/sdk";
import type { UiMessage } from "./chat.types.js";

/**
 * Higher-level chat actions that operate on an existing chat store.
 * Kept separate from the store so route files stay thin.
 */
export async function loadThreadMessages(
  sdk: ConusSdk,
  threadId: string
): Promise<ApiResult<UiMessage[]>> {
  const result = await sdk.threads.messages(threadId);
  if (result.error) return { data: null, error: result.error };
  const uiMessages: UiMessage[] = result.data.map((m, i) => ({
    id: `${threadId}-${i}`,
    role: m.role,
    content: m.content
  }));
  return { data: uiMessages, error: null };
}
