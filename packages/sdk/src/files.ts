import type { FileToken } from '@conusai/types';
import type { InternalClient } from './client.js';
import { EP } from './endpoints.js';
import type { ApiResult } from './types.js';

export function files(client: InternalClient) {
  return {
    async upload(file: File): Promise<ApiResult<FileToken>> {
      const token = await client.tokenProvider.get();
      const form = new FormData();
      form.append('file', file);
      try {
        const res = await client.fetch(`${client.baseUrl}${EP.FILES}`, {
          method: 'POST',
          headers: token ? { Authorization: `Bearer ${token}` } : {},
          body: form,
        });
        if (!res.ok) {
          return { data: null, error: { status: res.status, message: `HTTP ${res.status}` } };
        }
        return { data: await res.json() as FileToken, error: null };
      } catch (e: unknown) {
        return { data: null, error: { status: 0, message: e instanceof Error ? e.message : String(e) } };
      }
    },
  };
}
