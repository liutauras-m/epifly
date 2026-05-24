/**
 * auth.svelte.ts — Shell authentication state (Phase 3.1).
 *
 * Issues a compact HMAC-signed JWT stored in localStorage and injected as
 * both a cookie and the `x-session-token` header (via sdk.ts middleware).
 *
 * Intentionally minimal: name + plan only. Real IdP / OIDC login lives in
 * `apps/web` and eventually in the full Tauri PKCE flow (Phase 5.x).
 */

import { setSessionToken } from './sdk.js';
import { breadcrumbsStore, recentsStore, drawerStore } from '@conusai/ui/stores';

export interface ShellUser {
  name: string;
  plan: string;
}

const STORAGE_USER  = 'conusai_shell_user';
const STORAGE_TOKEN = 'conusai_shell_token';

// ── Reactive auth class ──────────────────────────────────────────────────────
class Auth {
  user = $state<ShellUser | null>(null);
}
export const auth = new Auth();

// ── Session cookie issuance ──────────────────────────────────────────────────
async function issueSessionCookie(name: string, plan: string): Promise<void> {
  const UI_SESSION_KEY =
    import.meta.env.VITE_UI_SESSION_KEY ??
    'conusai-foundry-dev-secret-change-me-32b';

  const exp       = Math.floor(Date.now() / 1000) + 7 * 86_400;
  const payload   = JSON.stringify({ name, plan, role: 'user', exp });
  const payloadB64 = btoa(payload)
    .replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');

  const key = await crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(UI_SESSION_KEY),
    { name: 'HMAC', hash: 'SHA-256' },
    false,
    ['sign'],
  );
  const sig = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(payloadB64));
  const mac = btoa(String.fromCharCode(...new Uint8Array(sig)))
    .replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');

  const token = `${payloadB64}.${mac}`;
  setSessionToken(token);
  localStorage.setItem(STORAGE_TOKEN, token);

  const apiBase = import.meta.env.VITE_API_BASE ?? '';
  const domain  = apiBase ? new URL(apiBase).hostname : 'localhost';
  document.cookie = `conusai_session=${token}; path=/; domain=${domain}; SameSite=Lax`;
}

// ── Public API ───────────────────────────────────────────────────────────────

/** Restore user + session from localStorage on app mount. */
export async function initAuth(): Promise<void> {
  // Restore cached token immediately so API calls work before the user object loads.
  const cachedToken = localStorage.getItem(STORAGE_TOKEN);
  if (cachedToken) setSessionToken(cachedToken);

  const raw = localStorage.getItem(STORAGE_USER);
  if (raw) {
    try {
      const stored = JSON.parse(raw) as ShellUser;
      if (stored?.name) {
        auth.user = stored;
        issueSessionCookie(stored.name, stored.plan).catch(() => {});
      }
    } catch { /* corrupt storage — ignore */ }
  }
}

/** Log in with a name + plan tier. Issues a new session cookie. */
export async function login(name: string, plan: string): Promise<void> {
  auth.user = { name, plan };
  localStorage.setItem(STORAGE_USER, JSON.stringify({ name, plan }));
  await issueSessionCookie(name, plan);
}

/**
 * Log out: clear auth state, session token, and UI stores.
 * Call `chatStream.newSession()` in the page's `onLogout` handler
 * if you also want to wipe the in-flight chat state.
 */
export function logout(): void {
  localStorage.removeItem(STORAGE_USER);
  localStorage.removeItem(STORAGE_TOKEN);
  setSessionToken(null);
  auth.user = null;

  // Reset navigation stores so the next login starts clean.
  breadcrumbsStore.clear();
  recentsStore.clear();
  drawerStore.close();
}
