import type { CapabilityCard } from '@conusai/types';
import type { InternalClient } from './client.js';
import type { ApiResult } from './types.js';
import { EP } from './endpoints.js';

export interface RegisterCapabilityRequest {
  capability_id: string;
  kind: string;
  endpoint: string;
  tools: unknown[];
  tenant_scope: string[];
}

export function capabilities(client: InternalClient) {
  return {
    list(): Promise<ApiResult<CapabilityCard[]>> {
      return client.call('GET', EP.CAPABILITIES);
    },

    search(q: string, limit = 10): Promise<ApiResult<CapabilityCard[]>> {
      return client.call('GET', `${EP.CAPABILITIES_SEARCH}?q=${encodeURIComponent(q)}&limit=${limit}`);
    },

    register(manifest: RegisterCapabilityRequest): Promise<ApiResult<void>> {
      return client.call('POST', '/admin/capabilities/register', manifest);
    },
  };
}
