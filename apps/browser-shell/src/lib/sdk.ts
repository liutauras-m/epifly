import { createConusSdk } from '@conusai/sdk';
import { invoke } from '@tauri-apps/api/core';

// Reads the device token from Rust state on every call (set via set_device_token command).
const tauriTokenProvider = {
  async get(): Promise<string | null> {
    try {
      return await invoke<string | null>('get_device_token');
    } catch {
      return null;
    }
  },
};

export const sdk = createConusSdk({
  fetch: globalThis.fetch.bind(globalThis),
  baseUrl: '',
  tokenProvider: tauriTokenProvider,
});
