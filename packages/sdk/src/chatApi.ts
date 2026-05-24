import { streamChat } from './chat.js';
import type { InternalClient } from './client.js';
import type { StreamChatParams } from './chat.js';

export function chatApi(_client: InternalClient) {
  return {
    stream(params: Omit<StreamChatParams, 'fetch' | 'baseUrl'>, opts?: { reconnect?: boolean }) {
      return streamChat({ ...params, baseUrl: _client.baseUrl, fetch: _client.fetch }, opts);
    },
  };
}
