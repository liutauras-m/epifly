/**
 * The in-flight OIDC login: state, nonce, PKCE verifier and where to return,
 * sealed into the short-lived host-only cookie `__Host-epifly_oidc`.
 */
import type { Cookies } from '@sveltejs/kit';
import { config } from '../config.js';
import { seal, unseal } from './seal.js';

export const TX_COOKIE = '__Host-epifly_oidc';
const TX_LIFETIME_S = 600;

export interface OidcTransaction {
	state: string;
	nonce: string;
	verifier: string;
	returnTo: string;
	/** Expiry, epoch seconds. */
	exp: number;
}

const OPTS = { path: '/', httpOnly: true, secure: true, sameSite: 'lax' } as const;

const TX_PURPOSE = 'epifly.oidc-tx.v1'; // gitleaks:allow (a seal purpose label, not a secret)

export function beginTransaction(cookies: Cookies, tx: Omit<OidcTransaction, 'exp'>): void {
	const value: OidcTransaction = { ...tx, exp: Math.floor(Date.now() / 1000) + TX_LIFETIME_S };
	cookies.set(TX_COOKIE, seal(config().sessionKey, TX_PURPOSE, value), { ...OPTS, maxAge: TX_LIFETIME_S });
}

/** Read and delete the transaction cookie. null when absent, tampered or expired. */
export function takeTransaction(cookies: Cookies): OidcTransaction | null {
	const raw = cookies.get(TX_COOKIE);
	if (raw === undefined) return null;
	cookies.delete(TX_COOKIE, OPTS);
	const tx = unseal<OidcTransaction>(config().sessionKey, TX_PURPOSE, raw);
	if (!tx || tx.exp <= Math.floor(Date.now() / 1000)) return null;
	return tx;
}

/** Only same-origin paths are accepted as a post-login destination. */
export function sanitizeReturnTo(raw: string | null): string {
	if (!raw || !raw.startsWith('/')) return '/';
	const base = 'http://return-to.invalid';
	let url: URL;
	try {
		url = new URL(raw, base);
	} catch {
		return '/';
	}
	if (url.origin !== base || url.pathname.startsWith('/auth/')) return '/';
	return `${url.pathname}${url.search}${url.hash}`;
}
