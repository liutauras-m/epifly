import adapter from '@sveltejs/adapter-node';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter(),
		// SvelteKit's origin check stays on for form posts; the gateway proxy
		// applies its own same-origin check to every non-GET request.
		// Nonce-based CSP: a fresh nonce per response covers the inline
		// hydration script. The browser only ever talks to its own origin —
		// API calls are proxied server-side, so connect-src is 'self' only.
		csp: {
			mode: 'nonce',
			directives: {
				'default-src': ['self'],
				'connect-src': ['self'],
				'img-src': ['self', 'data:', 'blob:'],
				'script-src': ['self'],
				'style-src': ['self', 'unsafe-inline'],
				'font-src': ['self', 'data:'],
				'object-src': ['none'],
				'base-uri': ['self'],
				'frame-ancestors': ['none'],
			},
		},
	},
};

export default config;
