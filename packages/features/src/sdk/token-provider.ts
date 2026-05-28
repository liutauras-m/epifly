import type { TokenProvider } from "@conusai/sdk";

const WEB_ACCESS_TOKEN_KEY = "epifly.web.access_token";

function getSessionStorage() {
  if (typeof globalThis.sessionStorage === "undefined") return null;
  return globalThis.sessionStorage;
}

export function setWebAccessToken(token: string) {
  getSessionStorage()?.setItem(WEB_ACCESS_TOKEN_KEY, token);
}

export function clearWebAccessToken() {
  getSessionStorage()?.removeItem(WEB_ACCESS_TOKEN_KEY);
}

/**
 * Web token provider.
 * For web, prefer server-managed auth/cookies where possible.
 * Do not store long-lived tokens in localStorage.
 * Replace this stub with real auth token retrieval.
 */
export function createWebTokenProvider(): TokenProvider {
  return {
    async get() {
      return getSessionStorage()?.getItem(WEB_ACCESS_TOKEN_KEY) ?? null;
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
