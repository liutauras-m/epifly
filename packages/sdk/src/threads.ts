import type { InternalClient } from './client.js';
import type { ApiResult } from './types.js';
import { EP } from './endpoints.js';

export interface ThreadMessage {
  role: 'user' | 'assistant';
  content: string;
  created_at: string;
}

/** Thread summary returned by `GET /v1/threads`. */
export interface ThreadSummary {
  id: string;
  title?: string;
  /** ISO-8601 timestamp; absent when the thread has only just been created. */
  last_active?: string;
  /** Number of messages in the thread. */
  message_count?: number;
}

export function threads(client: InternalClient) {
  return {
    list(opts: { limit?: number; after?: string } = {}): Promise<ApiResult<ThreadSummary[]>> {
      const params = new URLSearchParams();
      if (opts.limit) params.set('limit', String(opts.limit));
      if (opts.after) params.set('after', opts.after);
      const qs = params.toString() ? `?${params.toString()}` : '';
      // Backend wraps in { "data": [...] } (OpenAI-compatible envelope).
      // Unwrap so callers receive the array directly.
      return client.call<{ data: ThreadSummary[] }>('GET', `${EP.THREADS}${qs}`)
        .then(r => r.error
          ? { data: null, error: r.error }
          : { data: r.data?.data ?? [], error: null },
        );
    },
    messages(threadId: string): Promise<ApiResult<ThreadMessage[]>> {
      // Backend wraps in { "data": [...] } (OpenAI-compatible envelope). Unwrap.
      return client.call<{ data: ThreadMessage[] }>('GET', EP.THREAD_MESSAGES(threadId))
        .then(r => r.error
          ? { data: null, error: r.error }
          : { data: r.data?.data ?? [], error: null },
        );
    },
  };
}
