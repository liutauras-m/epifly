/**
 * POST /auth/logout — end the session: revoke the refresh token at Zitadel
 * (best effort), clear the cookie and hand over to Zitadel's end_session
 * endpoint. POST only, so a cross-site link cannot sign the user out
 * (SvelteKit's origin check covers form posts).
 */
import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { config } from '$lib/server/config.js';
import { client, oidc } from '$lib/server/auth/oidc.js';
import { clearSession, storedRefreshToken } from '$lib/server/auth/session.js';

export const POST: RequestHandler = async ({ cookies }) => {
	const refreshToken = storedRefreshToken(cookies);
	clearSession(cookies);

	const cfg = await oidc();
	if (refreshToken) {
		await client.tokenRevocation(cfg, refreshToken).catch((e: unknown) => {
			console.warn('[auth] refresh token revocation failed:', e instanceof Error ? e.name : 'unknown');
		});
	}

	if (!cfg.serverMetadata().end_session_endpoint) redirect(303, '/auth/login');
	const endSession = client.buildEndSessionUrl(cfg, {
		client_id: config().webClientId,
		post_logout_redirect_uri: config().postLogoutRedirectUri,
	});
	redirect(303, endSession.href);
};
