import type { ConusSdk } from "@conusai/sdk";
import type { ThreadSummary, ThreadMessage } from "@conusai/sdk";

export function createThreadsStore(sdk: ConusSdk) {
  let threads = $state<ThreadSummary[]>([]);
  let isLoading = $state(false);
  let hasLoaded = $state(false);
  let error = $state<string | null>(null);
  let realtimeSocket: WebSocket | null = null;
  let realtimeRefreshInFlight = false;
  let pendingRealtimeRefresh = false;

  async function load(opts: { limit?: number } = {}) {
    if (isLoading) return;
    isLoading = true;
    error = null;
    const result = await sdk.threads.list({ limit: opts.limit ?? 50 });
    isLoading = false;
    if (result.error) {
      error = result.error.message;
    } else {
      threads = result.data;
      hasLoaded = true;
    }
  }

  /** Load only if not already loaded. Use for initial mount. */
  async function loadOnce(opts: { limit?: number } = {}) {
    if (hasLoaded || isLoading) return;
    return load(opts);
  }

  async function loadMessages(threadId: string): Promise<ThreadMessage[]> {
    const result = await sdk.threads.messages(threadId);
    if (result.error) {
      error = result.error.message;
      return [];
    }
    return result.data;
  }

  async function waitForIdleLoad() {
    while (isLoading) {
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
  }

  async function refreshFromRealtime() {
    if (realtimeRefreshInFlight) {
      pendingRealtimeRefresh = true;
      return;
    }

    realtimeRefreshInFlight = true;
    try {
      do {
        pendingRealtimeRefresh = false;
        await waitForIdleLoad();
        await load({ limit: 20 });
      } while (pendingRealtimeRefresh);
    } finally {
      realtimeRefreshInFlight = false;
    }
  }

  function isThreadsChangeMessage(data: unknown) {
    if (!data || typeof data !== "object") return false;
    const record = data as Record<string, unknown>;
    return (
      (typeof record.op === "string" && record.op.startsWith("threads.")) ||
      record.resource === "threads" ||
      record.type === "threads"
    );
  }

  function connectRealtime() {
    if (realtimeSocket) return;

    realtimeSocket = sdk.realtime.subscribe();
    realtimeSocket.addEventListener("message", (event) => {
      try {
        const data = JSON.parse(typeof event.data === "string" ? event.data : "{}");
        if (isThreadsChangeMessage(data)) void refreshFromRealtime();
      } catch {
        // Ignore malformed realtime messages.
      }
    });
  }

  function disconnectRealtime() {
    realtimeSocket?.close();
    realtimeSocket = null;
  }

  return {
    get threads() { return threads; },
    get isLoading() { return isLoading; },
    get hasLoaded() { return hasLoaded; },
    get error() { return error; },
    load,
    loadOnce,
    loadMessages,
    connectRealtime,
    disconnectRealtime
  };
}
