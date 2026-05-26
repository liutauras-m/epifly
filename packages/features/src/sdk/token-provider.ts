import type { TokenProvider } from "@conusai/sdk";

/**
 * Web token provider.
 * For web, prefer server-managed auth/cookies where possible.
 * Do not store long-lived tokens in localStorage.
 * Replace this stub with real auth token retrieval.
 */
export function createWebTokenProvider(): TokenProvider {
  return {
    async get() {
      // TODO: fetch from session cookie or secure server endpoint.
      return null;
    }
  };
}

/**
 * Native token provider placeholder.
 * The final implementation should read from a scoped native storage abstraction.
 */
export function createNativeTokenProvider(): TokenProvider {
  return {
    async get() {
      return null;
    }
  };
}
