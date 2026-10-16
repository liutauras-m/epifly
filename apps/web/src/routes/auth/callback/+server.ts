/**
 * GET /auth/callback — finish the Zitadel login: check the sealed transaction,
 * exchange the code (state, nonce and PKCE are verified by openid-client) and
 * start the encrypted cookie session.
 */
import { error, redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { config } from '$lib/server/config.js';
import { client, oidc } from '$lib/server/auth/oidc.js';
import { createSession } from '$lib/server/auth/session.js';
import { takeTransaction } from '$lib/server/auth/transaction.js';

export const GET: RequestHandler = async ({ url, cookies }) => {
	const tx = takeTransaction(cookies);
	if (!tx) error(400, 'Sign-in expired or was started in another browser. Please sign in again.');

	const idpError = url.searchParams.get('error');
	if (idpError) error(400, `Sign-in failed: ${url.searchParams.get('error_description') ?? idpError}`);

	const cfg = await oidc();
	// openid-client compares the redirect URI it sees with the registered one, so
	// rebuild the callback URL on ORIGIN (the request may arrive via a proxy hop).
	const callbackUrl = new URL(config().redirectUri);
	callbackUrl.search = url.search;

	let tokens: Awaited<ReturnType<typeof client.authorizationCodeGrant>>;
	try {
		tokens = await client.authorizationCodeGrant(cfg, callbackUrl, {
			pkceCodeVerifier: tx.verifier,
			expectedState: tx.state,
			expectedNonce: tx.nonce,
			idTokenExpected: true,
		});
	} catch (e) {
		console.warn('[auth] code exchange failed:', e instanceof Error ? e.name : 'unknown');
		error(400, 'Sign-in failed. Please try again.');
	}

	const claims = tokens.claims();
	if (!claims?.sub) error(400, 'Sign-in failed: the identity provider returned no subject.');
	if (!tokens.refresh_token) {
		error(500, 'Sign-in failed: no refresh token. Enable the refresh token grant on the Zitadel app.');
	}

	const name =
		(typeof claims.name === 'string' && claims.name) ||
		(typeof claims.preferred_username === 'string' && claims.preferred_username) ||
		(typeof claims.email === 'string' && claims.email) ||
		claims.sub;

	createSession(
		cookies,
		{ sub: claims.sub, name, email: typeof claims.email === 'string' ? claims.email : null },
		{
			at: tokens.access_token,
			atExp: Math.floor(Date.now() / 1000) + (tokens.expires_in ?? 300),
			rt: tokens.refresh_token,
		},
	);

	redirect(303, tx.returnTo);
};
