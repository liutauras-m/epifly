import type { InternalClient } from './client.js';

const BASE_DELAY_MS = 500;
const MAX_DELAY_MS = 30_000;

export function realtime(client: InternalClient) {
  return {
    subscribe(): WebSocket {
      let ws: WebSocket | null = null;
      let delay = BASE_DELAY_MS;
      let closed = false;
      const listeners = new Map<string, Set<EventListenerOrEventListenerObject>>();

      function attachListeners(socket: WebSocket) {
        for (const [type, typeListeners] of listeners) {
          for (const listener of typeListeners) {
            socket.addEventListener(type, listener);
          }
        }
      }

      function connect() {
        // Resolve to an absolute WebSocket URL.
        // When baseUrl is already absolute (http(s)://) we just swap the scheme.
        // When baseUrl is relative (e.g. "/api") we build the WS URL from the
        // page origin — the path `/api/realtime/workspace` is appended to the
        // origin so that a Vite / reverse-proxy can forward it as a WS upgrade.
        let url: string;
        if (/^https?:\/\//.test(client.baseUrl)) {
          url = client.baseUrl.replace(/^http/, 'ws') + '/api/realtime/workspace';
        } else {
          const origin =
            typeof globalThis.location !== 'undefined'
              ? globalThis.location.origin
              : 'ws://localhost';
          url = origin.replace(/^http/, 'ws') + '/api/realtime/workspace';
        }
        ws = new WebSocket(url);
        attachListeners(ws);
        ws.addEventListener('open', () => { delay = BASE_DELAY_MS; });
        ws.addEventListener('close', () => {
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
          if (prop === 'addEventListener') {
            return (type: string, listener: EventListenerOrEventListenerObject) => {
              const typeListeners = listeners.get(type) ?? new Set<EventListenerOrEventListenerObject>();
              typeListeners.add(listener);
              listeners.set(type, typeListeners);
              ws?.addEventListener(type, listener);
            };
          }
          if (prop === 'removeEventListener') {
            return (type: string, listener: EventListenerOrEventListenerObject) => {
              listeners.get(type)?.delete(listener);
              ws?.removeEventListener(type, listener);
            };
          }
          if (prop === 'close') return () => { closed = true; ws?.close(); };
          if (prop === 'readyState') return ws?.readyState ?? WebSocket.CLOSED;
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          const val = (ws as any)?.[prop];
          return typeof val === 'function' ? val.bind(ws) : val;
        },
      });
    },
  };
}
