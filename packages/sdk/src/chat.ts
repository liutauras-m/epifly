import { EP } from './endpoints.js';
import type { ChatStreamDelta } from './types.js';

export interface StreamChatParams {
  message: string;
  threadId?: string | null;
  workspaceNodeId?: string | null;
  fetch?: typeof globalThis.fetch;
  signal?: AbortSignal;
}

const BACKOFF = [200, 600, 1800];

export async function* streamChat(
  params: StreamChatParams,
  opts: { reconnect?: boolean } = {}
): AsyncGenerator<ChatStreamDelta> {
  const { reconnect = true } = opts;
  const fetchFn = params.fetch ?? globalThis.fetch;
  let attempts = 0;

  while (true) {
    try {
      const body: Record<string, unknown> = { message: params.message };
      if (params.threadId) body.thread_id = params.threadId;
      if (params.workspaceNodeId) body.workspace_node_id = params.workspaceNodeId;

      const res = await fetchFn(EP.UI_STREAM, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
        signal: params.signal,
      });

      if (!res.ok || !res.body) throw new Error(`HTTP ${res.status}`);
      attempts = 0;

      const reader = res.body.getReader();
      const dec = new TextDecoder();
      let buf = '';

      while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        buf += dec.decode(value, { stream: true });
        let pos: number;
        while ((pos = buf.indexOf('\n\n')) !== -1) {
          const block = buf.slice(0, pos);
          buf = buf.slice(pos + 2);
          for (const line of block.split('\n')) {
            if (!line.startsWith('data: ')) continue;
            const raw = line.slice(6);
            if (raw === '[DONE]') { yield { kind: 'done' }; return; }
            let ev: Record<string, unknown>;
            try { ev = JSON.parse(raw); } catch { continue; }

            const delta = (ev.choices as { delta?: Record<string, unknown> }[])?.[0]?.delta;
            if (delta) {
              if (typeof delta.content === 'string') {
                yield { kind: 'text', content: delta.content };
              } else if (delta.tool_call_start) {
                const { id, name } = delta.tool_call_start as { id: string; name: string };
                yield { kind: 'tool_start', id, name };
              } else if (delta.tool_call_result) {
                const { tool_use_id, result } = delta.tool_call_result as { tool_use_id: string; result: string };
                yield { kind: 'tool_result', tool_use_id, result };
              }
            }

            const tid = ev.thread_id as string | null;
            if (tid) yield { kind: 'thread_id', id: tid };
          }
        }
      }
      return;
    } catch (e: unknown) {
      if (!reconnect || attempts >= BACKOFF.length) throw e;
      const delay = BACKOFF[attempts++];
      await new Promise(r => setTimeout(r, delay));
    }
  }
}
