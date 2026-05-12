/**
 * GET /auth/callback — Zitadel OIDC callback handler.
 * Exchanges the authorization code for tokens and sets session cookie.
 */
import { redirect, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { exchangeCode, COOKIE_NAME } from '$lib/server/oidc.js';

export const GET: RequestHandler = async ({ url, cookies }) => {
	const code = url.searchParams.get('code');
	const state = url.searchParams.get('state');
	const errorParam = url.searchParams.get('error');

	if (errorParam) {
		const desc = url.searchParams.get('error_description') ?? errorParam;
		error(400, `Authentication failed: ${desc}`);
	}

	if (!code) {
		error(400, 'Missing authorization code');
	}

	const storedState = cookies.get('oidc_state');
	const verifier = cookies.get('oidc_verifier');

	if (!storedState || state !== storedState) {
		error(400, 'Invalid OAuth state');
	}
	if (!verifier) {
		error(400, 'Missing PKCE verifier');
	}

	cookies.delete('oidc_state', { path: '/' });
	cookies.delete('oidc_verifier', { path: '/' });

	const tokens = await exchangeCode(code, verifier);

	// Store the access_token as the session cookie (verified on every request).
	cookies.set(COOKIE_NAME, tokens.access_token, {
		path: '/',
		httpOnly: true,
		sameSite: 'lax',
		maxAge: tokens.expires_in ?? 3600,
		secure: process.env.NODE_ENV === 'production',
	});

	redirect(302, '/');
};
