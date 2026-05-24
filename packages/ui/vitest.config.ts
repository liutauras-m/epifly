import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [
    svelte({
      hot: false,
      onwarn(warning, handler) {
        // css_unused_selector: sibling selector (.X + .X) is unreachable in Svelte's static analysis
        // when the selector spans component instances — it works at runtime. Suppress the false-positive.
        if (warning.code === 'css_unused_selector') return;
        handler(warning);
      },
    }),
  ],
  test: {
    environment: 'jsdom',
    globals: true,
    include: ['tests/**/*.test.ts'],
    setupFiles: ['tests/setup.ts'],
  },
});
