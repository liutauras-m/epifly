/**
 * Browser session of the backend-for-frontend.
 *
 * The session is the Zitadel token set, sealed with AES-256-GCM
 * (AUTH_SESSION_KEY) into the host-only cookie `__Host-epifly_session`
 * (Secure, HttpOnly, SameSite=Lax, Path=/). No server-side store: the cookie
 * is the session. The ID token is not kept (logout uses `client_id` instead of
 * `id_token_hint`), which keeps the cookie at roughly 1.5–2.5 KB for a JWT
 * access token. A sealed value larger than 3800 bytes is split over
 * `__Host-epifly_session.1` and `.2`; anything larger is refused.
 *
 * The access token is refreshed with the refresh token when it has less than
 * a minute left. Refreshes are single-flight per refresh token within the
 * process, and a completed refresh is remembered for 30 seconds so parallel
 * requests that still carry the old cookie reuse the rotated tokens instead
 * of presenting a refresh token Zitadel has already rotated away.
 */
import type { Cookies } from '@sveltejs/kit';
import { createHash } from 'node:crypto';
import { config } from '../config.js';
import { client, oidc } from './oidc.js';
import { seal, unseal } from './seal.js';

export const SESSION_COOKIE = '__Host-epifly_session';
const CHUNK_BYTES = 3800;
const MAX_CHUNKS = 3;
/** Absolute session lifetime; the refresh token may not outlive it. */
const SESSION_LIFETIME_S = 30 * 24 * 3600;
const REFRESH_SKEW_S = 60;
const REMEMBER_REFRESH_MS = 30_000;

export interface SessionUser {
	sub: string;
	name: string;
	email: string | null;
}

/** What request handlers see: the user and a currently valid access token. */
export interface Session {
	user: SessionUser;
	accessToken: string;
}

interface StoredSession {
	v: 1;
	user: SessionUser;
	at: string;
	/** Access token expiry, epoch seconds. */
	atExp: number;
	rt: string;
	/** Session creation, epoch seconds — bounds the absolute lifetime. */
	iat: number;
}

interface TokenSet {
	at: string;
	atExp: number;
	rt: string;
}

const COOKIE_OPTS = {
	path: '/',
	httpOnly: true,
	secure: true,
	sameSite: 'lax',
} as const;

const now = () => Math.floor(Date.now() / 1000);

function chunkName(i: number): string {
	return i === 0 ? SESSION_COOKIE : `${SESSION_COOKIE}.${i}`;
}

function readStored(cookies: Cookies): StoredSession | null {
	const head = cookies.get(SESSION_COOKIE);
	if (!head) return null;
	let sealed = head;
	const sep = head.indexOf('~');
	if (sep > 0) {
		const n = Number(head.slice(0, sep));
		if (!Number.isInteger(n) || n < 2 || n > MAX_CHUNKS) return null;
		const parts = [head.slice(sep + 1)];
		for (let i = 1; i < n; i++) {
			const part = cookies.get(chunkName(i));
			if (!part) return null;
			parts.push(part);
		}
		sealed = parts.join('');
	}
	const stored = unseal<StoredSession>(config().sessionKey, 'epifly.session.v1', sealed);
	if (!stored || stored.v !== 1 || typeof stored.rt !== 'string' || typeof stored.at !== 'string') return null;
	if (stored.iat + SESSION_LIFETIME_S <= now()) return null;
	return stored;
}

function writeStored(cookies: Cookies, stored: StoredSession): void {
	const sealed = seal(config().sessionKey, 'epifly.session.v1', stored);
	const n = Math.ceil(sealed.length / CHUNK_BYTES);
	if (n > MAX_CHUNKS) throw new Error(`[session] sealed session is ${sealed.length} bytes; the limit is ${CHUNK_BYTES * MAX_CHUNKS}`);
	const maxAge = Math.max(0, stored.iat + SESSION_LIFETIME_S - now());
	for (let i = 0; i < n; i++) {
		const part = sealed.slice(i * CHUNK_BYTES, (i + 1) * CHUNK_BYTES);
		cookies.set(chunkName(i), i === 0 && n > 1 ? `${n}~${part}` : part, { ...COOKIE_OPTS, maxAge });
	}
	for (let i = n; i < MAX_CHUNKS; i++) {
		if (cookies.get(chunkName(i)) !== undefined) cookies.delete(chunkName(i), COOKIE_OPTS);
	}
}

export function clearSession(cookies: Cookies): void {
	for (let i = 0; i < MAX_CHUNKS; i++) {
		if (cookies.get(chunkName(i)) !== undefined) cookies.delete(chunkName(i), COOKIE_OPTS);
	}
}

/** Start a session from a successful authorization-code exchange. */
export function createSession(cookies: Cookies, user: SessionUser, tokens: TokenSet): void {
	writeStored(cookies, { v: 1, user, ...tokens, iat: now() });
}

/** Refresh token of the current session (for revocation at logout), without refreshing. */
export function storedRefreshToken(cookies: Cookies): string | null {
	return readStored(cookies)?.rt ?? null;
}

// ── Refresh (single-flight) ───────────────────────────────────────────────────

const inflight = new Map<string, Promise<TokenSet | null>>();
const remembered = new Map<string, { tokens: TokenSet; until: number }>();

function keyOf(refreshToken: string): string {
	return createHash('sha256').update(refreshToken).digest('base64url');
}

/** Exchange a refresh token. null = the grant is dead (invalid_grant); throws on transient failures. */
async function exchangeRefreshToken(refreshToken: string): Promise<TokenSet | null> {
	const cfg = await oidc();
	try {
		const res = await client.refreshTokenGrant(cfg, refreshToken);
		return {
			at: res.access_token,
			atExp: now() + (res.expires_in ?? 300),
			rt: res.refresh_token ?? refreshToken,
		};
	} catch (e) {
		if (e instanceof client.ResponseBodyError && (e.error === 'invalid_grant' || e.error === 'invalid_client')) {
			return null;
		}
		throw e;
	}
}

export function refreshSingleFlight(
	refreshToken: string,
	exchange: (rt: string) => Promise<TokenSet | null> = exchangeRefreshToken,
): Promise<TokenSet | null> {
	const key = keyOf(refreshToken);
	const t = Date.now();
	const hit = remembered.get(key);
	if (hit && hit.until > t) return Promise.resolve(hit.tokens);

	let pending = inflight.get(key);
	if (!pending) {
		pending = exchange(refreshToken)
			.then((tokens) => {
				if (tokens) {
					for (const [k, v] of remembered) if (v.until <= Date.now()) remembered.delete(k);
					remembered.set(key, { tokens, until: Date.now() + REMEMBER_REFRESH_MS });
				}
				return tokens;
			})
			.finally(() => inflight.delete(key));
		inflight.set(key, pending);
	}
	return pending;
}

/**
 * Decrypt the session cookie and make sure its access token is usable,
 * refreshing (and re-writing the cookie) when it is about to expire.
 * Returns null when there is no valid session; a dead session is cleared.
 */
export async function resolveSession(cookies: Cookies): Promise<Session | null> {
	const stored = readStored(cookies);
	if (!stored) {
		if (cookies.get(SESSION_COOKIE) !== undefined) clearSession(cookies);
		return null;
	}
	if (stored.atExp - now() > REFRESH_SKEW_S) {
		return { user: stored.user, accessToken: stored.at };
	}

	let tokens: TokenSet | null;
	try {
		tokens = await refreshSingleFlight(stored.rt);
	} catch (e) {
		console.warn('[session] token refresh failed:', e instanceof Error ? e.message : 'unknown');
		// Transient failure: keep the session; use the old token while it still lives.
		return stored.atExp > now() ? { user: stored.user, accessToken: stored.at } : null;
	}
	if (!tokens) {
		clearSession(cookies);
		return null;
	}
	writeStored(cookies, { ...stored, ...tokens });
	return { user: stored.user, accessToken: tokens.at };
}

/** Test hook: forget in-flight and remembered refreshes. */
export function _resetRefreshState(): void {
	inflight.clear();
	remembered.clear();
}
