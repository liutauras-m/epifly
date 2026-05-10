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

// VITE_API_BASE is baked in at build time (set in .env.local or as a build env var).
// Defaults to '' (same-origin) which only works in dev proxy mode; always set it for iOS/desktop builds.
const API_BASE = import.meta.env.VITE_API_BASE ?? '';

export const sdk = createConusSdk({
  fetch: globalThis.fetch.bind(globalThis),
  baseUrl: API_BASE,
  tokenProvider: tauriTokenProvider,
});
