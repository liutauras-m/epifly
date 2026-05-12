/**
 * GET /auth — redirect to Zitadel authorization endpoint.
 * Only active when AUTH_PROVIDER=zitadel.
 */
import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import {
	buildAuthUrl,
	generateCodeVerifier,
	generateCodeChallenge,
} from '$lib/server/oidc.js';

export const GET: RequestHandler = async ({ cookies }) => {
	if (process.env.AUTH_PROVIDER !== 'zitadel') {
		redirect(302, '/login');
	}

	const verifier = generateCodeVerifier();
	const challenge = generateCodeChallenge(verifier);
	const state = crypto.randomUUID();

	// Store verifier + state in a short-lived cookie (5 min).
	cookies.set('oidc_verifier', verifier, {
		path: '/',
		httpOnly: true,
		sameSite: 'lax',
		maxAge: 300,
		secure: process.env.NODE_ENV === 'production',
	});
	cookies.set('oidc_state', state, {
		path: '/',
		httpOnly: true,
		sameSite: 'lax',
		maxAge: 300,
		secure: process.env.NODE_ENV === 'production',
	});

	redirect(302, buildAuthUrl(state, challenge));
};
