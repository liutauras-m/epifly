import { randomBytes } from 'node:crypto';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const KEY = randomBytes(32).toString('base64');
Object.assign(process.env, {
	ORIGIN: 'https://app.example.test',
	GATEWAY_URL: 'http://gateway:8080',
	ZITADEL_ISSUER: 'https://auth.example.test',
	ZITADEL_WEB_CLIENT_ID: 'web-client',
	AUTH_SESSION_KEY: KEY,
});

const { loadConfig } = await import('$lib/server/config.js');
const { seal, unseal } = await import('$lib/server/auth/seal.js');
const { sanitizeReturnTo } = await import('$lib/server/auth/transaction.js');
const session = await import('$lib/server/auth/session.js');

/** Minimal in-memory implementation of SvelteKit's Cookies. */
function jar(initial: Record<string, string> = {}) {
	const store = new Map(Object.entries(initial));
	const writes: { name: string; value: string; opts: Record<string, unknown> }[] = [];
	return {
		store,
		writes,
		get: (name: string) => store.get(name),
		getAll: () => [...store].map(([name, value]) => ({ name, value })),
		set: (name: string, value: string, opts: Record<string, unknown>) => {
			store.set(name, value);
			writes.push({ name, value, opts });
		},
		delete: (name: string) => {
			store.delete(name);
		},
		serialize: () => '',
	};
}

describe('config', () => {
	const base = {
		ORIGIN: 'https://app.example.test',
		GATEWAY_URL: 'http://gateway:8080',
		ZITADEL_ISSUER: 'https://auth.example.test',
		ZITADEL_WEB_CLIENT_ID: 'web-client',
		AUTH_SESSION_KEY: KEY,
	};

	it('derives the redirect URIs from ORIGIN', () => {
		const cfg = loadConfig(base);
		expect(cfg.redirectUri).toBe('https://app.example.test/auth/callback');
		expect(cfg.postLogoutRedirectUri).toBe('https://app.example.test/auth/signed-out');
		expect(cfg.webClientSecret).toBeNull();
	});

	it.each(['ORIGIN', 'GATEWAY_URL', 'ZITADEL_ISSUER', 'ZITADEL_WEB_CLIENT_ID', 'AUTH_SESSION_KEY'])(
		'refuses to start without %s',
		(name) => {
			expect(() => loadConfig({ ...base, [name]: undefined })).toThrow(name);
		},
	);

	it('refuses a session key that is not 32 bytes', () => {
		expect(() => loadConfig({ ...base, AUTH_SESSION_KEY: randomBytes(16).toString('base64') })).toThrow(/32 bytes/);
	});

	it('refuses a plain-http issuer unless it is loopback', () => {
		expect(() => loadConfig({ ...base, ZITADEL_ISSUER: 'http://auth.example.test' })).toThrow(/https/);
		expect(loadConfig({ ...base, ZITADEL_ISSUER: 'http://127.0.0.1:4180' }).issuer.port).toBe('4180');
	});
});

describe('seal', () => {
	const key = randomBytes(32);

	it('round-trips a value', () => {
		const sealed = seal(key, 'epifly.session.v1', { a: 1 });
		expect(unseal(key, 'epifly.session.v1', sealed)).toEqual({ a: 1 });
	});

	it('rejects a value sealed for another purpose', () => {
		const sealed = seal(key, 'epifly.oidc-tx.v1', { a: 1 }); // gitleaks:allow (forged test value)
		expect(unseal(key, 'epifly.session.v1', sealed)).toBeNull();
	});

	it('rejects a tampered value and a wrong key', () => {
		const sealed = seal(key, 'epifly.session.v1', { a: 1 });
		const raw = Buffer.from(sealed, 'base64url');
		raw[14] ^= 1;
		expect(unseal(key, 'epifly.session.v1', raw.toString('base64url'))).toBeNull();
		expect(unseal(randomBytes(32), 'epifly.session.v1', sealed)).toBeNull();
	});

	it('rejects an unsigned JWT-shaped cookie (S14)', () => {
		const fakeJwt = 'eyJhbGciOiJub25lIn0.eyJzdWIiOiJhZG1pbiIsImV4cCI6OTk5OTk5OTk5OX0.'; // gitleaks:allow (forged test value)
		expect(unseal(key, 'epifly.session.v1', fakeJwt)).toBeNull();
	});
});

describe('sanitizeReturnTo', () => {
	it.each([
		[null, '/'],
		['/account?tab=1', '/account?tab=1'],
		['//evil.example', '/'],
		['/\\evil.example', '/'],
		['https://evil.example/', '/'],
		['/auth/login', '/'],
	])('%s → %s', (input, expected) => {
		expect(sanitizeReturnTo(input)).toBe(expected);
	});
});

describe('session cookie', () => {
	const user = { sub: 'u1', name: 'Ada', email: 'ada@example.test' };
	const future = () => Math.floor(Date.now() / 1000) + 3600;

	beforeEach(() => session._resetRefreshState());

	it('writes a host-only, secure, http-only cookie and reads it back', async () => {
		const cookies = jar();
		session.createSession(cookies as never, user, { at: 'access-1', atExp: future(), rt: 'refresh-1' });
		const w = cookies.writes[0];
		expect(w.name).toBe('__Host-epifly_session');
		expect(w.opts).toMatchObject({ path: '/', httpOnly: true, secure: true, sameSite: 'lax' });
		expect(w.opts.domain).toBeUndefined();
		expect(w.value).not.toContain('access-1');
		await expect(session.resolveSession(cookies as never)).resolves.toEqual({ user, accessToken: 'access-1' });
	});

	it('splits an oversized session over chunk cookies and reassembles it', async () => {
		const cookies = jar();
		const bigToken = 'x'.repeat(5000);
		session.createSession(cookies as never, user, { at: bigToken, atExp: future(), rt: 'refresh-1' });
		expect(cookies.store.has('__Host-epifly_session.1')).toBe(true);
		const resolved = await session.resolveSession(cookies as never);
		expect(resolved?.accessToken).toBe(bigToken);
	});

	it('treats a garbage cookie as signed out and clears it', async () => {
		const cookies = jar({ '__Host-epifly_session': 'not-a-session' });
		await expect(session.resolveSession(cookies as never)).resolves.toBeNull();
		expect(cookies.store.has('__Host-epifly_session')).toBe(false);
	});
});

describe('refreshSingleFlight', () => {
	beforeEach(() => session._resetRefreshState());

	it('runs one exchange for concurrent callers and remembers the result', async () => {
		const exchange = vi.fn(async (rt: string) => {
			await new Promise((r) => setTimeout(r, 10));
			return { at: `at-for-${rt}`, atExp: 1, rt: 'rotated' };
		});
		const results = await Promise.all([
			session.refreshSingleFlight('rt-1', exchange),
			session.refreshSingleFlight('rt-1', exchange),
			session.refreshSingleFlight('rt-1', exchange),
		]);
		expect(exchange).toHaveBeenCalledTimes(1);
		expect(new Set(results.map((r) => r?.at))).toEqual(new Set(['at-for-rt-1']));
		// A late request still carrying the old refresh token reuses the rotation.
		await session.refreshSingleFlight('rt-1', exchange);
		expect(exchange).toHaveBeenCalledTimes(1);
	});

	it('does not remember a dead grant', async () => {
		const exchange = vi.fn(async () => null);
		await expect(session.refreshSingleFlight('rt-dead', exchange)).resolves.toBeNull();
		await session.refreshSingleFlight('rt-dead', exchange);
		expect(exchange).toHaveBeenCalledTimes(2);
	});
});
