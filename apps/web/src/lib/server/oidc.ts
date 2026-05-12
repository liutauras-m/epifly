/**
 * Zitadel OIDC adapter — activated when AUTH_PROVIDER=zitadel.
 *
 * Implements the SessionAdapter interface so call-sites are unchanged.
 * Uses the standard authorization-code + PKCE flow with Zitadel.
 */
import crypto from 'node:crypto';
import type { SessionAdapter, SessionUser } from './session.js';

const COOKIE_NAME = 'conusai_oidc_session';
const TOKEN_COOKIE = 'conusai_access_token';

function cfg(key: string, fallback?: string): string {
	const v = process.env[key] ?? fallback;
	if (!v) throw new Error(`[oidc] ${key} is required`);
	return v;
}

function domain(): string {
	return cfg('ZITADEL_DOMAIN');
}

function clientId(): string {
	return cfg('ZITADEL_CLIENT_ID');
}

function clientSecret(): string {
	return cfg('ZITADEL_CLIENT_SECRET', '');
}

function redirectUri(): string {
	const base = cfg('AUTH_REDIRECT_BASE', 'http://localhost:3000');
	return `${base}/auth/callback`;
}

// ── PKCE helpers ──────────────────────────────────────────────────────────────

function generateCodeVerifier(): string {
	return crypto.randomBytes(32).toString('base64url');
}

function generateCodeChallenge(verifier: string): string {
	return crypto.createHash('sha256').update(verifier).digest('base64url');
}

// ── Token exchange ────────────────────────────────────────────────────────────

interface TokenResponse {
	access_token: string;
	id_token?: string;
	refresh_token?: string;
	expires_in?: number;
	token_type?: string;
}

interface IdTokenClaims {
	sub?: string;
	email?: string;
	exp?: number;
	'urn:conusai:plan_tier'?: string;
	'urn:conusai:subscription_status'?: string;
}

export async function exchangeCode(code: string, codeVerifier: string): Promise<TokenResponse> {
	const url = `${domain()}/oauth/v2/token`;
	const body = new URLSearchParams({
		grant_type: 'authorization_code',
		code,
		redirect_uri: redirectUri(),
		client_id: clientId(),
		code_verifier: codeVerifier,
		...(clientSecret() ? { client_secret: clientSecret() } : {}),
	});

	const res = await fetch(url, {
		method: 'POST',
		headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
		body: body.toString(),
	});

	if (!res.ok) {
		const text = await res.text();
		throw new Error(`Token exchange failed: HTTP ${res.status} — ${text}`);
	}

	return res.json() as Promise<TokenResponse>;
}

export async function revokeToken(token: string): Promise<void> {
	const url = `${domain()}/oauth/v2/revoke`;
	const body = new URLSearchParams({
		token,
		client_id: clientId(),
		...(clientSecret() ? { client_secret: clientSecret() } : {}),
	});
	await fetch(url, {
		method: 'POST',
		headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
		body: body.toString(),
	});
}

// ── Auth URL builder ──────────────────────────────────────────────────────────

export function buildAuthUrl(state: string, codeChallenge: string): string {
	const params = new URLSearchParams({
		response_type: 'code',
		client_id: clientId(),
		redirect_uri: redirectUri(),
		scope: 'openid email profile',
		state,
		code_challenge: codeChallenge,
		code_challenge_method: 'S256',
	});
	return `${domain()}/oauth/v2/authorize?${params.toString()}`;
}

export function buildLogoutUrl(idToken?: string): string {
	const params = new URLSearchParams({
		client_id: clientId(),
		post_logout_redirect_uri: redirectUri().replace('/auth/callback', '/login'),
		...(idToken ? { id_token_hint: idToken } : {}),
	});
	return `${domain()}/oidc/v1/end_session?${params.toString()}`;
}

// ── Session adapter ───────────────────────────────────────────────────────────

export class ZitadelOidcAdapter implements SessionAdapter {
	async issue(_name: string, _plan: string): Promise<string> {
		throw new Error('ZitadelOidcAdapter: use /auth/callback to issue sessions');
	}

	async verify(cookie: string): Promise<SessionUser | null> {
		// The cookie holds the access_token JWT.
		const parts = cookie.split('.');
		if (parts.length !== 3) return null;
		try {
			const payload = JSON.parse(
				Buffer.from(parts[1], 'base64url').toString()
			) as IdTokenClaims;
			if (!payload.exp || payload.exp < Math.floor(Date.now() / 1000)) return null;
			return {
				name: payload.email ?? payload.sub ?? '',
				plan: payload['urn:conusai:plan_tier'] ?? 'free',
				exp: payload.exp,
			};
		} catch {
			return null;
		}
	}
}

export { generateCodeVerifier, generateCodeChallenge };
export { COOKIE_NAME, TOKEN_COOKIE };
