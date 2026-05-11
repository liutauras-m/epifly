import adapter from '@sveltejs/adapter-node';

const BACKEND_ORIGIN = new URL(process.env.CONUSAI_BACKEND_URL ?? 'http://localhost:8080').origin;

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter(),
		// Disable SvelteKit's blanket CSRF check; hooks.server.ts enforces it
		// only for browser-navigated form paths (not proxied backend fetches).
		// Also needed for WebKit (Safari / Playwright iOS) which omits the Origin
		// header on same-origin form submissions.
		csrf: { checkOrigin: false },
		// SvelteKit nonce-based CSP: generates a unique nonce per request and injects it
		// into all inline scripts (hydration data, etc.), then includes it in the header.
		// This replaces the manual header in hooks.server.ts.
		csp: {
			mode: 'nonce',
			directives: {
				'default-src': ['self'],
				'connect-src': ['self', 'wss:', BACKEND_ORIGIN],
				'img-src': ['self', 'data:', 'blob:'],
				'script-src': ['self'],
				'style-src': ['self', 'unsafe-inline'],
			},
		},
	},
};

export default config;
