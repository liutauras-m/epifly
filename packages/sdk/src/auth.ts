import type { InternalClient } from './client.js';
import type { ApiResult } from './types.js';

export function auth(client: InternalClient) {
  return {
    async login(email: string, password: string): Promise<void> {
      await client.request('POST', '/api/auth/login', { email, password });
    },
    async logout(): Promise<void> {
      await client.request('POST', '/api/auth/logout');
    },
    async verifyDeviceToken(token: string): Promise<ApiResult<void>> {
      return client.call('POST', '/api/auth/device/verify', { token });
    },
  };
}
