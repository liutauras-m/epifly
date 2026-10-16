import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	ssr: {
		// Force workspace packages through Vite's resolver instead of Node's
		// ESM loader chain (where @tailwindcss/node's loader can't map
		// the `.js` extensions in source `.ts` re-exports).
		noExternal: ['@conusai/sdk', '@conusai/ui', '@conusai/types'],
	},
	// No dev proxy: in `vite dev` the SvelteKit server itself proxies /v1, /api,
	// /ui, /mcp and /admin to GATEWAY_URL, exactly as in production.
	server: {
		port: 5173,
		fs: {
			allow: ['../../packages/ui'],
		},
	},
});
