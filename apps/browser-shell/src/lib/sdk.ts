import { createConusSdk } from '@conusai/sdk';
import { invoke } from '@tauri-apps/api/core';

// ── PKCE / system-browser helpers ────────────────────────────────────────────

/** Open a URL in the OS default browser (Stripe Checkout, IdP login, etc.). */
export async function openInSystemBrowser(url: string): Promise<void> {
  await invoke('open_in_system_browser', { url });
}

export interface PkceResult {
  code: string;
  state: string | null;
  redirect_uri: string;
  code_verifier: string;
}

/**
 * Full PKCE login flow.
 * Opens `authUrl` in the system browser, waits for the local callback,
 * and returns the auth code + verifier for server-side token exchange.
 */
export async function pkceLogin(authUrl: string, extraParams?: string): Promise<PkceResult> {
  return await invoke<PkceResult>('pkce_login', {
    authUrl,
    extraParams: extraParams ?? null,
  });
}

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

// Shared session token for /ui/* cross-origin auth.
// WKWebView can't send cookies cross-origin to http:// endpoints (Secure flag blocks it),
// so we inject the HMAC token as X-Session-Token header instead.
let _sessionToken: string | null = null;
export function setSessionToken(token: string | null) { _sessionToken = token; }
export function getSessionToken(): string | null { return _sessionToken; }

// VITE_API_BASE is baked in at build time (set in .env.local or as a build env var).
// Defaults to '' (same-origin) which only works in dev proxy mode; always set it for iOS/desktop builds.
const API_BASE = import.meta.env.VITE_API_BASE ?? '';

export const sdk = createConusSdk({
  fetch: (url, init) => {
    // Resolve relative paths against API_BASE so callers that pass a bare
    // path (e.g. /ui/stream) reach the gateway, not the Tauri asset server.
    const resolvedUrl =
      typeof url === 'string' && url.startsWith('/') ? `${API_BASE}${url}` : url;
    if (_sessionToken) {
      const headers = new Headers(init?.headers);
      headers.set('x-session-token', _sessionToken);
      return globalThis.fetch(resolvedUrl, { ...init, headers });
    }
    return globalThis.fetch(resolvedUrl, init);
  },
  baseUrl: API_BASE,
  tokenProvider: tauriTokenProvider,
});
