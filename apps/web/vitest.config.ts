import { defineConfig } from 'vitest/config';
import { fileURLToPath } from 'node:url';

const r = (p: string) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig({
	resolve: {
		alias: {
			$lib: r('./src/lib'),
			'$env/dynamic/private': r('./src/tests/stubs/env-dynamic-private.ts'),
		},
	},
	test: {
		include: ['src/tests/**/*.{test,spec}.ts'],
		environment: 'node',
	},
});
