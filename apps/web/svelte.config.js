import adapter from "@sveltejs/adapter-auto";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter(),
    alias: {
      "@conusai/sdk": "../../packages/sdk/src/index.ts",
      "@conusai/types": "../../packages/types/src/index.ts",
      "@epifly/ui": "../../packages/ui/src/index.ts",
      "@epifly/ui/*": "../../packages/ui/src/*",
      "@epifly/features": "../../packages/features/src/index.ts",
      "@epifly/features/*": "../../packages/features/src/*",
      "@epifly/shared": "../../packages/shared/src/index.ts",
      "@epifly/shared/*": "../../packages/shared/src/*"
    }
  }
};

export default config;
