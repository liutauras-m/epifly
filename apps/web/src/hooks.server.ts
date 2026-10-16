import { redirect, type Handle, type ServerInit } from '@sveltejs/kit';
import { sequence } from '@sveltejs/kit/hooks';
import { dev } from '$app/environment';
import { config, initConfig } from '$lib/server/config.js';
import { resolveSession } from '$lib/server/auth/session.js';
import { isProxiedPath } from '$lib/server/proxy.js';

/** Refuse to start without a complete, valid environment (see $lib/server/config.ts). */
export const init: ServerInit = () => {
	const cfg = initConfig();
	console.info(`[web] origin=${cfg.origin.origin} gateway=${cfg.gatewayUrl.origin} issuer=${cfg.issuer.href}`);
};

/** Paths reachable without a session. Proxied paths answer 401 themselves. */
function isPublic(pathname: string): boolean {
	if (pathname === '/healthz' || pathname.startsWith('/auth/')) return true;
	// Component gallery: dev builds only (its layout redirects away in production).
	return dev && (pathname === '/_/ui' || pathname.startsWith('/_/ui/'));
}

const session: Handle = async ({ event, resolve }) => {
	const { pathname } = event.url;
	event.locals.session = null;
	if (pathname === '/healthz' || pathname.startsWith('/auth/')) return resolve(event);

	event.locals.session = await resolveSession(event.cookies);

	if (!event.locals.session && !isPublic(pathname) && !isProxiedPath(pathname)) {
		redirect(303, `/auth/login?returnTo=${encodeURIComponent(pathname + event.url.search)}`);
	}
	return resolve(event, {
		transformPageChunk({ html }) {
			// Vite content-hashes font files, so the hashed URL is only known at
			// render time: preload the Geist faces the stylesheet references.
			const m = html.match(/\/_app\/immutable\/assets\/Geist-Variable\.[^"']+\.woff2/);
			const mMono = html.match(/\/_app\/immutable\/assets\/GeistMono-Variable\.[^"']+\.woff2/);
			const preloads = [
				m ? `<link rel="preload" as="font" type="font/woff2" crossorigin href="${m[0]}">` : '',
				mMono ? `<link rel="preload" as="font" type="font/woff2" crossorigin href="${mMono[0]}">` : '',
			]
				.filter(Boolean)
				.join('\n\t');
			if (!preloads) return html;
			return html.replace('%sveltekit.head%', `${preloads}\n\t%sveltekit.head%`);
		},
	});
};

const securityHeaders: Handle = async ({ event, resolve }) => {
	const response = await resolve(event);
	const set = (name: string, value: string) => {
		if (!response.headers.has(name)) response.headers.set(name, value);
	};
	try {
		set('x-content-type-options', 'nosniff');
		set('x-frame-options', 'DENY');
		set('referrer-policy', 'strict-origin-when-cross-origin');
		set('permissions-policy', 'camera=(), microphone=(), geolocation=(), payment=()');
		if (config().origin.protocol === 'https:') set('strict-transport-security', 'max-age=31536000; includeSubDomains');
	} catch {
		// Immutable headers (a fetch() Response passed through untouched) — leave as is.
	}
	return response;
};

export const handle = sequence(securityHeaders, session);
