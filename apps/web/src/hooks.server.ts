import type { Handle } from '@sveltejs/kit';
import { COOKIE_NAME, verify } from '$lib/server/session.js';

// Paths routed to the backend — exempt from SvelteKit CSRF origin check
// (they are fetch() calls, not form submissions, so SvelteKit wouldn't check
// them anyway; but we keep this list explicit for clarity and future proofing).
const CSRF_EXEMPT_PREFIXES = ['/v1', '/api', '/ui', '/mcp', '/admin'];

// NOTE: Production key enforcement (throw on missing UI_SESSION_KEY) lives in
// src/lib/server/session.ts → getKey(). It fires on the first sign/verify call.

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

	// ── Font preloads (Phase 2.2) ─────────────────────────────────────────────
	// Vite content-hashes font files, so the hashed URL is only known at
	// render time. We scan the <head> for the first font-face link Vite
	// generated and emit a matching preload before it so the browser can start
	// fetching Geist before it parses the stylesheet.
	return resolve(event, {
		transformPageChunk({ html }) {
			// Find the first Geist woff2 URL already injected by Vite (looks like
			// /_app/immutable/assets/Geist-Variable.HASH.woff2)
			const m = html.match(/\/_app\/immutable\/assets\/Geist-Variable\.[^"']+\.woff2/);
			const mMono = html.match(/\/_app\/immutable\/assets\/GeistMono-Variable\.[^"']+\.woff2/);
			const preloads = [
				m     ? `<link rel="preload" as="font" type="font/woff2" crossorigin href="${m[0]}">` : '',
				mMono ? `<link rel="preload" as="font" type="font/woff2" crossorigin href="${mMono[0]}">` : '',
			].filter(Boolean).join('\n\t');
			if (!preloads) return html;
			return html.replace('%sveltekit.head%', `${preloads}\n\t%sveltekit.head%`);
		},
	});
};
