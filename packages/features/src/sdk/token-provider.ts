import type { TokenProvider } from "@conusai/sdk";

/**
 * Web token provider — returns null because the SvelteKit BFF proxy injects
 * the Authorization header server-side. The browser never holds a token.
 */
export function createWebTokenProvider(): TokenProvider {
  return {
    async get() {
      return null;
    },
  };
}

/**
 * Native token provider — will be implemented in Phase 5 (OS keychain + Tauri
 * command bridge). Until then returns null; native auth is gated by Phase 4.
 */
export function createNativeTokenProvider(): TokenProvider {
  return {
    async get() {
      return null;
    },
  };
}
