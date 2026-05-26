import type { TokenProvider } from "@conusai/sdk";

/**
 * Native token provider — reads from secure native storage.
 * TODO: replace with @tauri-apps/plugin-store or OS keychain integration.
 */
export function createNativeTokenProvider(): TokenProvider {
  return {
    async get() {
      // TODO: read from native secure storage abstraction.
      // Do NOT use localStorage as final implementation.
      return null;
    }
  };
}
