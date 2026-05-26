import type { InternalClient } from './client.js';
import { EP } from './endpoints.js';
import type { ApiResult } from './types.js';

export function auth(client: InternalClient) {
  return {
    async login(email: string, password: string): Promise<void> {
      await client.request('POST', EP.AUTH_LOGIN, { email, password });
    },
    async logout(): Promise<void> {
      await client.request('POST', EP.AUTH_LOGOUT);
    },
    async verifyDeviceToken(token: string): Promise<ApiResult<void>> {
      return client.call('POST', EP.AUTH_DEVICE_VERIFY, { token });
    },
  };
}
