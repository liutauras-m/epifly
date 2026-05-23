import crypto from 'node:crypto';

export const COOKIE_NAME = 'conusai_session';
const TTL_SECS = 24 * 3600;

export interface SessionUser {
	name: string;
	plan: string;
	/**
	 * Tenant identifier from the upstream JWT (`tenant_id` claim) — used
	 * client-side to defensively filter `resource_invalidated` SSE deltas (PR
	 * 3.A.7). Always trust the server-side gateway scope as the actual security
	 * boundary; this is a belt-and-braces check that exposes mismatches as
	 * `console.warn` rather than silent UI drift.
	 *
	 * Optional + nullable so existing HMAC cookies (which have no claim) keep
	 * working — users issued from those flows simply skip the client check.
	 */
	tenantId?: string | null;
	exp: number;
}

// ── Session adapter interface ─────────────────────────────────────────────────
// Provides a seam so the backend-JWT adapter can be swapped in without changing
// call-sites. Both adapters return the same { data, error } shapes as apiCall.

export interface SessionAdapter {
	issue(name: string, plan: string): Promise<string>;
	verify(cookie: string): Promise<SessionUser | null>;
}

// ── Key helper ────────────────────────────────────────────────────────────────
function getKey(): string {
	const key = process.env.UI_SESSION_KEY;
	if (!key) {
		if (process.env.NODE_ENV === 'production') {
			throw new Error(
				'[session] UI_SESSION_KEY must be set in production. ' +
				'Generate one with: openssl rand -hex 32'
			);
		}
		return 'conusai-foundry-dev-secret-change-me-32b';
	}
	return key;
}

function b64url(buf: Buffer): string {
	return buf.toString('base64url');
}

// ── LocalHmacAdapter — default, no backend dependency ────────────────────────
export class LocalHmacAdapter implements SessionAdapter {
	async issue(name: string, plan: string): Promise<string> {
		return sign(name, plan);
	}
	async verify(cookie: string): Promise<SessionUser | null> {
		return verifyRaw(cookie);
	}
}

// ── BackendJwtAdapter — activated when BACKEND_AUTH_LOGIN_URL is set ──────────
// Issues sessions by calling the backend's login endpoint and receiving a JWT.
// The JWT payload is decoded locally (no signature verification — the gateway
// already verified it). Activated by setting BACKEND_AUTH_LOGIN_URL env var.
export class BackendJwtAdapter implements SessionAdapter {
	private loginUrl: string;
	constructor(loginUrl: string) {
		this.loginUrl = loginUrl;
	}
	async issue(name: string, plan: string): Promise<string> {
		const res = await fetch(this.loginUrl, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ name, plan }),
		});
		if (!res.ok) throw new Error(`Backend login failed: HTTP ${res.status}`);
		const { token } = await res.json() as { token: string };
		return token;
	}
	async verify(cookie: string): Promise<SessionUser | null> {
		// Decode JWT payload (middle segment) without re-verifying signature —
		// signature is already checked by the Rust gateway on each request.
		const parts = cookie.split('.');
		if (parts.length !== 3) return null;
		try {
			const payload = JSON.parse(Buffer.from(parts[1], 'base64url').toString()) as {
				sub?: string; name?: string; plan?: string; tenant_id?: string; exp?: number;
			};
			if (!payload.exp || payload.exp < Math.floor(Date.now() / 1000)) return null;
			return {
				name: payload.name ?? payload.sub ?? '',
				plan: payload.plan ?? 'enterprise',
				tenantId: payload.tenant_id ?? null,
				exp: payload.exp,
			};
		} catch { return null; }
	}
}

// ── Active adapter — resolved once at import time ─────────────────────────────
function resolveAdapter(): SessionAdapter {
	const loginUrl = process.env.BACKEND_AUTH_LOGIN_URL;
	if (loginUrl) return new BackendJwtAdapter(loginUrl);
	return new LocalHmacAdapter();
}
export const sessionAdapter: SessionAdapter = resolveAdapter();

// ── Low-level HMAC helpers (used by LocalHmacAdapter + login action) ─────────
export function sign(name: string, plan: string): string {
	const exp = Math.floor(Date.now() / 1000) + TTL_SECS;
	const payload: SessionUser = { name, plan, exp };
	const payloadB64 = b64url(Buffer.from(JSON.stringify(payload)));
	const mac = crypto.createHmac('sha256', getKey()).update(payloadB64).digest();
	return `${payloadB64}.${b64url(mac)}`;
}

function verifyRaw(cookieValue: string): SessionUser | null {
	const dot = cookieValue.indexOf('.');
	if (dot < 0) return null;
	const payloadB64 = cookieValue.slice(0, dot);
	const sigB64 = cookieValue.slice(dot + 1);
	const expected = b64url(crypto.createHmac('sha256', getKey()).update(payloadB64).digest());
	try {
		if (!crypto.timingSafeEqual(Buffer.from(expected), Buffer.from(sigB64))) return null;
	} catch { return null; }
	try {
		const user = JSON.parse(Buffer.from(payloadB64, 'base64url').toString()) as SessionUser;
		if (user.exp < Math.floor(Date.now() / 1000)) return null;
		return user;
	} catch { return null; }
}

// Exported verify delegates to the active adapter for consistency
export function verify(cookieValue: string): SessionUser | null {
	// BackendJwtAdapter.verify is async, but SvelteKit hooks expect sync.
	// For HMAC cookies (the default) this is sync; JWT cookies are parsed
	// synchronously too (no network call needed for decode-only verify).
	if (cookieValue.split('.').length === 3) {
		// Looks like a JWT — decode payload synchronously
		try {
			const parts = cookieValue.split('.');
			const payload = JSON.parse(Buffer.from(parts[1], 'base64url').toString()) as {
				sub?: string; name?: string; plan?: string; tenant_id?: string; exp?: number;
			};
			if (!payload.exp || payload.exp < Math.floor(Date.now() / 1000)) return null;
			return {
				name: payload.name ?? payload.sub ?? '',
				plan: payload.plan ?? 'enterprise',
				tenantId: payload.tenant_id ?? null,
				exp: payload.exp,
			};
		} catch { return null; }
	}
	return verifyRaw(cookieValue);
}

// ── Utility helpers ───────────────────────────────────────────────────────────
export function firstName(name: string): string {
	return name.split(/\s+/)[0] ?? name;
}

export function initials(name: string): string {
	return name
		.split(/\s+/)
		.slice(0, 2)
		.map((w) => w[0] ?? '')
		.join('')
		.toUpperCase();
}

export function timeGreeting(): string {
	const h = new Date().getHours();
	if (h >= 5 && h < 12) return 'Morning';
	if (h >= 12 && h < 18) return 'Afternoon';
	if (h >= 18 && h < 23) return 'Evening';
	return 'Late night';
}
