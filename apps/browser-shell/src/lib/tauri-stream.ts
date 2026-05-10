// Tauri IPC–based SSE streaming.
// WKWebView buffers HTTP responses completely before exposing them to JS
// (confirmed: tauri-apps/plugins-workspace#2415, #2129).
// This module routes streaming through Rust → Tauri events instead.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ChatStreamDelta } from '@conusai/sdk';

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const API_BASE = (import.meta.env.VITE_API_BASE as string | undefined) ?? 'http://localhost:8080';

interface ChunkPayload {
  kind: 'text' | 'tool_start' | 'tool_result' | 'thread_id' | 'done' | 'error';
  content?: string;
  id?: string;
  name?: string;
  tool_use_id?: string;
  result?: string;
  message?: string;
}

export async function* streamChatTauri(params: {
  message: string;
  sessionToken: string;
  threadId?: string | null;
  workspaceNodeId?: string | null;
  signal?: AbortSignal;
}): AsyncGenerator<ChatStreamDelta> {
  const streamId = await invoke<string>('chat_stream_start', {
    message: params.message,
    sessionToken: params.sessionToken,
    threadId: params.threadId ?? null,
    workspaceNodeId: params.workspaceNodeId ?? null,
    apiBase: API_BASE,
  });

  const deltas: ChatStreamDelta[] = [];
  let wakeup: (() => void) | null = null;
  let ended = false;
  const unlisteners: UnlistenFn[] = [];

  const notify = () => {
    const w = wakeup;
    wakeup = null;
    w?.();
  };

  unlisteners.push(
    await listen<ChunkPayload>(`chat:chunk:${streamId}`, (ev) => {
      const p = ev.payload;
      switch (p.kind) {
        case 'text':
          if (p.content) deltas.push({ kind: 'text', content: p.content });
          break;
        case 'tool_start':
          if (p.id && p.name) deltas.push({ kind: 'tool_start', id: p.id, name: p.name });
          break;
        case 'tool_result':
          if (p.tool_use_id) deltas.push({ kind: 'tool_result', tool_use_id: p.tool_use_id, result: p.result ?? '' });
          break;
        case 'thread_id':
          if (p.id) deltas.push({ kind: 'thread_id', id: p.id });
          break;
        case 'done':
          ended = true;
          break;
        case 'error':
          deltas.push({ kind: 'text', content: `Error: ${p.message ?? 'unknown'}` });
          ended = true;
          break;
      }
      notify();
    }),
  );

  params.signal?.addEventListener('abort', () => {
    invoke('chat_stream_abort', { streamId }).catch(() => {});
    ended = true;
    notify();
  });

  try {
    while (true) {
      while (deltas.length > 0) yield deltas.shift()!;
      if (ended) break;
      await new Promise<void>((r) => { wakeup = r; });
    }
    while (deltas.length > 0) yield deltas.shift()!;
    yield { kind: 'done' };
  } finally {
    for (const u of unlisteners) u();
  }
}
