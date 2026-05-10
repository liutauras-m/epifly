import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";
import { viteStaticCopy } from "vite-plugin-static-copy";
import { fileURLToPath } from "url";
import { join, dirname } from "path";

const uiAssets = join(
  dirname(fileURLToPath(import.meta.url)),
  "../../packages/ui/src/lib/assets"
);

export default defineConfig({
  plugins: [
    sveltekit(),
    viteStaticCopy({
      targets: [
        { src: `${uiAssets}/images/*`, dest: "images" },
        { src: `${uiAssets}/icons/*`, dest: "icons" },
        { src: `${uiAssets}/fonts/*`, dest: "fonts" },
      ],
    }),
  ],
  clearScreen: false,
  server: {
    port: 5174,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
