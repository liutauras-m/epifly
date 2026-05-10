import { streamChat } from './chat.js';
import type { InternalClient } from './client.js';
import type { StreamChatParams } from './chat.js';

export function chatApi(_client: InternalClient) {
  return {
    stream(params: Omit<StreamChatParams, 'fetch'>, opts?: { reconnect?: boolean }) {
      return streamChat({ ...params, fetch: _client.fetch }, opts);
    },
  };
}
