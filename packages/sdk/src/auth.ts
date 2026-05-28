import type { InternalClient } from './client.js';
import { EP } from './endpoints.js';
import type { ApiResult } from './types.js';

export type LoginResponse = {
  access_token: string;
  token_type: string;
  expires_in: number;
  tenant_id: string;
};

export function auth(client: InternalClient) {
  return {
    async login(email: string, password: string): Promise<LoginResponse> {
      return client.request<LoginResponse>('POST', EP.AUTH_LOGIN, { email, password });
    },
    async logout(): Promise<void> {
      await client.request('POST', EP.AUTH_LOGOUT);
    },
    async verifyDeviceToken(token: string): Promise<ApiResult<void>> {
      return client.call('POST', EP.AUTH_DEVICE_VERIFY, { token });
    },
  };
}
