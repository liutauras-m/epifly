import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';

const BACKEND = process.env.CONUSAI_BACKEND_URL ?? 'http://localhost:8080';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	ssr: {
		// Force workspace packages through Vite's resolver instead of Node's
		// ESM loader chain (where @tailwindcss/node's loader can't map
		// the `.js` extensions in source `.ts` re-exports).
		noExternal: ['@conusai/sdk', '@conusai/ui', '@conusai/types'],
	},
	server: {
		port: 5173,
		fs: {
			allow: ['../../packages/ui'],
		},
		proxy: {
			'/v1': { target: BACKEND, changeOrigin: true },
			'/api': { target: BACKEND, changeOrigin: true },
			'/admin': { target: BACKEND, changeOrigin: true },
			'/ui': { target: BACKEND, changeOrigin: true },
			'/swagger-ui': { target: BACKEND, changeOrigin: true },
			'/docs': { target: BACKEND, changeOrigin: true },
			'/openapi.json': { target: BACKEND, changeOrigin: true },
			'/metrics': { target: BACKEND, changeOrigin: true }
		}
	},
	test: {
		include: ['src/tests/**/*.{test,spec}.ts'],
		environment: 'node',
	}
});
