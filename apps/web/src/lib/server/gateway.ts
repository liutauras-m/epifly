/**
 * Server-side gateway access for load functions and form actions: calls
 * GATEWAY_URL directly with the session's bearer token.
 */
import { redirect } from '@sveltejs/kit';
import { createConusSdk } from '@conusai/sdk';
import { config } from './config.js';
import type { Session } from './auth/session.js';

const SSR_TIMEOUT_MS = 10_000;

/** The session, or a redirect to sign-in (defence in depth behind the hook guard). */
export function requireSession(locals: App.Locals, returnTo = '/'): Session {
	if (!locals.session) redirect(303, `/auth/login?returnTo=${encodeURIComponent(returnTo)}`);
	return locals.session;
}

/** fetch() bound to the gateway origin and the session's access token. */
export function gatewayFetch(session: Session): typeof fetch {
	const base = config().gatewayUrl;
	return ((input: string | URL, init: RequestInit = {}) => {
		const url = new URL(String(input), base);
		if (url.origin !== base.origin) throw new Error('gatewayFetch only reaches GATEWAY_URL');
		const headers = new Headers(init.headers);
		headers.set('authorization', `Bearer ${session.accessToken}`);
		if (init.body != null && !headers.has('content-type')) headers.set('content-type', 'application/json');
		return fetch(url, { ...init, headers, signal: init.signal ?? AbortSignal.timeout(SSR_TIMEOUT_MS) });
	}) as typeof fetch;
}

/** The typed SDK over gatewayFetch, for SSR loads. */
export function serverSdk(session: Session) {
	return createConusSdk({
		fetch: gatewayFetch(session),
		baseUrl: '',
		tokenProvider: { get: async () => null },
	});
}
