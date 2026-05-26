import type { ConusSdk } from "@conusai/sdk";
import type { ThreadSummary, ThreadMessage } from "@conusai/sdk";

export function createThreadsStore(sdk: ConusSdk) {
  let threads = $state<ThreadSummary[]>([]);
  let isLoading = $state(false);
  let error = $state<string | null>(null);

  async function load(opts: { limit?: number } = {}) {
    isLoading = true;
    error = null;
    const result = await sdk.threads.list({ limit: opts.limit ?? 50 });
    isLoading = false;
    if (result.error) {
      error = result.error.message;
    } else {
      threads = result.data;
    }
  }

  async function loadMessages(threadId: string): Promise<ThreadMessage[]> {
    const result = await sdk.threads.messages(threadId);
    if (result.error) {
      error = result.error.message;
      return [];
    }
    return result.data;
  }

  return {
    get threads() { return threads; },
    get isLoading() { return isLoading; },
    get error() { return error; },
    load,
    loadMessages
  };
}
