import type { ControlMessage } from '@conusai/types';
import type { InternalClient } from './client.js';

export function shells(client: InternalClient) {
  return {
    control(deviceId: string): WebSocket {
      const url = client.baseUrl.replace(/^http/, 'ws') + `/v1/shells/${deviceId}/control`;
      return new WebSocket(url);
    },
    parseMessage(data: string): ControlMessage {
      return JSON.parse(data) as ControlMessage;
    },
  };
}
