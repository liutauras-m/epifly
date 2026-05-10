import type { InternalClient } from './client.js';
import type { ApiResult, InvoiceData, UploadResponse } from './types.js';
import { EP } from './endpoints.js';

export function ui(client: InternalClient) {
  return {
    extractInvoice(fileId: string): Promise<ApiResult<InvoiceData>> {
      return client.call('POST', EP.UI_EXTRACT_INVOICE, { file_id: fileId });
    },

    async upload(file: File): Promise<ApiResult<UploadResponse>> {
      const token = await client.tokenProvider.get();
      const fd = new FormData();
      fd.append('file', file, file.name);
      try {
        const res = await client.fetch(`${client.baseUrl}${EP.UI_UPLOAD}`, {
          method: 'POST',
          headers: token ? { Authorization: `Bearer ${token}` } : {},
          body: fd,
        });
        if (!res.ok) {
          let message = `HTTP ${res.status}`;
          try { const j = await res.json(); message = (j as { error?: string }).error ?? message; } catch {}
          return { data: null, error: { status: res.status, message } };
        }
        return { data: await res.json() as UploadResponse, error: null };
      } catch (e: unknown) {
        return { data: null, error: { status: 0, message: e instanceof Error ? e.message : String(e) } };
      }
    },
  };
}
