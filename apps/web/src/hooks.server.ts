import type { Handle } from '@sveltejs/kit';
import { COOKIE_NAME, verify } from '$lib/server/session';

// Paths routed to the backend — exempt from SvelteKit CSRF origin check
// (they are fetch() calls, not form submissions, so SvelteKit wouldn't check
// them anyway; but we keep this list explicit for clarity and future proofing).
const CSRF_EXEMPT_PREFIXES = ['/v1', '/api', '/ui', '/mcp', '/admin'];

// Warn loudly in production if the session key is still the insecure default.
if (process.env.NODE_ENV === 'production') {
	const key = process.env.UI_SESSION_KEY ?? '';
	if (!key || key === 'conusai-foundry-dev-secret-change-me-32b') {
		console.error(
			'[hooks] FATAL: UI_SESSION_KEY is not set or uses the insecure default. ' +
			'Set a strong random secret before deploying.'
		);
	}
}

export const handle: Handle = async ({ event, resolve }) => {
	// ── Manual CSRF origin check (scoped) ────────────────────────────────────
	// We disabled SvelteKit's blanket csrf.checkOrigin in svelte.config.js so
	// that backend-proxied fetch() calls with a mismatched Origin still work.
	// Instead we enforce origin checking only for browser-navigated form paths.
	const { method, pathname } = { method: event.request.method, pathname: event.url.pathname };
	const isApiPath = CSRF_EXEMPT_PREFIXES.some(prefix => pathname.startsWith(prefix));

	if (!isApiPath && method !== 'GET' && method !== 'HEAD') {
		const origin = event.request.headers.get('origin');
		const host = event.request.headers.get('host');
		if (origin && host) {
			try {
				if (new URL(origin).host !== host) {
					return new Response('CSRF check failed', { status: 403 });
				}
			} catch {
				return new Response('CSRF check failed', { status: 403 });
			}
		}
	}

	// ── Session auth ─────────────────────────────────────────────────────────
	const raw = event.cookies.get(COOKIE_NAME);
	event.locals.user = raw ? (verify(raw) ?? null) : null;

	return resolve(event);
};
