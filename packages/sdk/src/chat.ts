import { EP } from './endpoints.js';
import type { ChatStreamDelta } from './types.js';

export interface StreamChatParams {
  message: string;
  baseUrl: string;
  threadId?: string | null;
  workspaceNodeId?: string | null;
  attachmentIds?: string[];
  /**
   * Optional capability name to pin for this turn (PR 2.A).
   *
   * When provided, the gateway prepends that capability's tools before any
   * semantic-routing hits so the LLM is guaranteed to see them.
   * Set by the "Invoke in current workspace" button in the UI.
   */
  forcedCapability?: string | null;
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
      if (params.attachmentIds?.length) body.attachment_ids = params.attachmentIds;
      if (params.forcedCapability) body.forced_capability = params.forcedCapability;

      const res = await fetchFn(`${params.baseUrl}${EP.UI_STREAM}`, {
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
                const error = result.startsWith('Error:') ? result.slice('Error:'.length).trim() : undefined;
                yield { kind: 'tool_result', tool_use_id, result, ...(error !== undefined && { error }) };
              } else if (delta.routing_meta) {
                const rm = delta.routing_meta as {
                  forced_capability: string | null;
                  selected_capabilities: string[];
                  pinned_tools: string[];
                  lexical_hits: string[];
                  max_score: number;
                };
                yield {
                  kind: 'routing_meta',
                  forced_capability: rm.forced_capability ?? null,
                  selected_capabilities: rm.selected_capabilities ?? [],
                  pinned_tools: rm.pinned_tools ?? [],
                  lexical_hits: rm.lexical_hits ?? [],
                  max_score: rm.max_score ?? 0,
                };
              } else if (delta.resource_invalidated) {
                const ri = delta.resource_invalidated as { resource: string; scope: string; changed_keys?: string[] };
                yield {
                  kind: 'resource_invalidated',
                  resource: ri.resource ?? '',
                  scope: ri.scope ?? '',
                  changed_keys: ri.changed_keys ?? [],
                };
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
