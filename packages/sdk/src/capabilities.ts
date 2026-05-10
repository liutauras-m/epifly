import type { CapabilityCard } from "@conusai/types";
import type { ConusaiClient } from "./client.js";

export interface RegisterCapabilityRequest {
  capability_id: string;
  kind: string;
  endpoint: string;
  tools: unknown[];
  tenant_scope: string[];
}

export function capabilities(client: ConusaiClient) {
  return {
    list(): Promise<CapabilityCard[]> {
      return client.request("GET", "/api/capabilities");
    },

    search(q: string, limit = 10): Promise<CapabilityCard[]> {
      return client.request("GET", `/api/capabilities/search?q=${encodeURIComponent(q)}&limit=${limit}`);
    },

    register(manifest: RegisterCapabilityRequest): Promise<void> {
      return client.request("POST", "/admin/capabilities/register", manifest);
    },
  };
}
