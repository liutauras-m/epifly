/**
 * Reactive wrapper around sdk.realtime.subscribe().
 *
 * Rules (from plan):
 * - Never open WebSocket connections directly in components.
 * - Wrap subscriptions inside feature stores.
 * - Close sockets in $effect cleanup.
 * - Show connection status in UI.
 */
import type { ConusSdk } from "@conusai/sdk";

export type RealtimeMessage = {
  type: string;
  payload: unknown;
};

export function createRealtimeStore(sdk: ConusSdk) {
  let isConnected = $state(false);
  let lastMessage = $state<RealtimeMessage | null>(null);
  let error = $state<string | null>(null);
  let ws: WebSocket | null = null;

  function connect() {
    if (ws) return; // already connected

    ws = sdk.realtime.subscribe();

    ws.addEventListener("open", () => {
      isConnected = true;
      error = null;
    });

    ws.addEventListener("close", () => {
      isConnected = false;
      ws = null;
      // sdk.realtime.subscribe() already handles reconnect with exponential backoff.
      // Do not reconnect here to avoid double-reconnect loops.
    });

    ws.addEventListener("error", () => {
      error = "Realtime connection error";
    });

    ws.addEventListener("message", (event) => {
      try {
        const data = JSON.parse(typeof event.data === "string" ? event.data : "{}");
        lastMessage = { type: data.type ?? "unknown", payload: data.payload ?? data };
      } catch {
        // Ignore malformed messages.
      }
    });
  }

  function disconnect() {
    ws?.close();
    ws = null;
    isConnected = false;
  }

  return {
    get isConnected() { return isConnected; },
    get lastMessage() { return lastMessage; },
    get error() { return error; },
    connect,
    disconnect
  };
}
