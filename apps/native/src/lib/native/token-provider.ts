import type { TokenProvider } from "@conusai/sdk";
import { getAccessToken } from "./auth.js";

/**
 * Native token provider — calls the Rust auth manager via Tauri command.
 * The Rust side proactively refreshes the token (60s before expiry) and
 * reads/writes from the OS keychain. JS never holds the refresh token.
 */
export function createNativeTokenProvider(): TokenProvider {
  return {
    async get() {
      try {
        return await getAccessToken();
      } catch {
        return null;
      }
    },
  };
}
