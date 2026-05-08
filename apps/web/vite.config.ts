import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	server: {
		port: 5173,
		proxy: {
			'/v1': { target: 'http://localhost:8080', changeOrigin: true },
			'/api': { target: 'http://localhost:8080', changeOrigin: true },
			'/admin': { target: 'http://localhost:8080', changeOrigin: true },
			'/ui/stream': { target: 'http://localhost:8080', changeOrigin: true },
			'/ui/upload': { target: 'http://localhost:8080', changeOrigin: true },
			'/ui/extract-invoice': { target: 'http://localhost:8080', changeOrigin: true },
			'/swagger-ui': { target: 'http://localhost:8080', changeOrigin: true },
			'/metrics': { target: 'http://localhost:8080', changeOrigin: true }
		}
	}
});
