import type { InternalClient } from './client.js';
import type { ApiResult } from './types.js';
import { EP } from './endpoints.js';

export interface ThreadMessage {
  role: 'user' | 'assistant';
  content: string;
  created_at: string;
}

export function threads(client: InternalClient) {
  return {
    list(opts: { limit?: number } = {}): Promise<ApiResult<{ id: string; title?: string }[]>> {
      const qs = opts.limit ? `?limit=${opts.limit}` : '';
      return client.call('GET', `${EP.THREADS}${qs}`);
    },
    messages(threadId: string): Promise<ApiResult<ThreadMessage[]>> {
      return client.call('GET', EP.THREAD_MESSAGES(threadId));
    },
  };
}
