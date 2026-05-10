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
    messages(threadId: string): Promise<ApiResult<ThreadMessage[]>> {
      return client.call('GET', EP.THREAD_MESSAGES(threadId));
    },
  };
}
