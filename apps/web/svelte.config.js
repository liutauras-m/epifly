import adapter from '@sveltejs/adapter-node';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter(),
		// SvelteKit nonce-based CSP: generates a unique nonce per request and injects it
		// into all inline scripts (hydration data, etc.), then includes it in the header.
		// This replaces the manual header in hooks.server.ts.
		csp: {
			mode: 'nonce',
			directives: {
				'default-src': ['self'],
				'connect-src': ['self', 'wss:'],
				'img-src': ['self', 'data:', 'blob:'],
				'script-src': ['self'],
				'style-src': ['self', 'unsafe-inline'],
			},
		},
	},
};

export default config;
