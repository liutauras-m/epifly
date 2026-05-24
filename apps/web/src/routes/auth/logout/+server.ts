/**
 * GET /auth/logout — RP-initiated logout from Zitadel.
 */
import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { buildLogoutUrl, revokeToken, COOKIE_NAME } from '$lib/server/oidc.js';

export const GET: RequestHandler = async ({ cookies }) => {
	const token = cookies.get(COOKIE_NAME);

	if (token) {
		// Best-effort token revocation; ignore errors.
		try { await revokeToken(token); } catch { /* ignore */ }
	}

	cookies.delete(COOKIE_NAME, { path: '/' });
	cookies.delete('conusai_session', { path: '/' });

	if (process.env.AUTH_PROVIDER === 'zitadel') {
		redirect(302, buildLogoutUrl(token));
	}
	redirect(302, '/login');
};
