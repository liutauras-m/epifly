import type { ConusaiClient } from "./client.js";

const BASE_DELAY_MS = 500;
const MAX_DELAY_MS = 30_000;

export function realtime(client: ConusaiClient) {
  return {
    subscribe(): WebSocket {
      let ws: WebSocket;
      let delay = BASE_DELAY_MS;
      let closed = false;

      function connect() {
        const url = client.baseUrl.replace(/^http/, "ws") + "/api/realtime/workspace";
        ws = new WebSocket(url);

        ws.addEventListener("open", () => {
          delay = BASE_DELAY_MS;
        });

        ws.addEventListener("close", () => {
          if (!closed) {
            const jitter = Math.random() * delay * 0.2;
            setTimeout(connect, delay + jitter);
            delay = Math.min(delay * 2, MAX_DELAY_MS);
          }
        });
      }

      connect();

      return new Proxy({} as WebSocket, {
        get(_, prop) {
          if (prop === "close") {
            return () => {
              closed = true;
              ws?.close();
            };
          }
          const val = (ws as never)[prop];
          return typeof val === "function" ? val.bind(ws) : val;
        },
      });
    },
  };
}
