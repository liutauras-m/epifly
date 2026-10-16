/**
 * GET /auth/login — start the Zitadel authorization-code flow (PKCE S256,
 * state, nonce). `?returnTo=/path` picks the page to land on afterwards.
 */
import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { config } from '$lib/server/config.js';
import { LOGIN_SCOPE, client, oidc } from '$lib/server/auth/oidc.js';
import { beginTransaction, sanitizeReturnTo } from '$lib/server/auth/transaction.js';

export const GET: RequestHandler = async ({ url, cookies }) => {
	const cfg = await oidc();
	const verifier = client.randomPKCECodeVerifier();
	const state = client.randomState();
	const nonce = client.randomNonce();

	beginTransaction(cookies, {
		state,
		nonce,
		verifier,
		returnTo: sanitizeReturnTo(url.searchParams.get('returnTo')),
	});

	const authUrl = client.buildAuthorizationUrl(cfg, {
		redirect_uri: config().redirectUri,
		scope: LOGIN_SCOPE,
		state,
		nonce,
		code_challenge: await client.calculatePKCECodeChallenge(verifier),
		code_challenge_method: 'S256',
	});
	redirect(303, authUrl.href);
};
