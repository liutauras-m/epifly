import type { ConusSdk } from "@conusai/sdk";
import type { CapabilityCard } from "@conusai/types";

export function createCapabilitiesStore(sdk: ConusSdk) {
  let capabilities = $state<CapabilityCard[]>([]);
  let isLoading = $state(false);
  let error = $state<string | null>(null);

  async function load() {
    isLoading = true;
    error = null;
    const result = await sdk.capabilities.list();
    isLoading = false;
    if (result.error) {
      error = result.error.message;
    } else {
      capabilities = result.data;
    }
  }

  async function search(q: string): Promise<CapabilityCard[]> {
    const result = await sdk.capabilities.search(q);
    if (result.error) {
      error = result.error.message;
      return [];
    }
    return result.data;
  }

  return {
    get capabilities() { return capabilities; },
    get isLoading() { return isLoading; },
    get error() { return error; },
    load,
    search
  };
}
