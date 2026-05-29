import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  server: {
    proxy: {
      // Proxy the realtime WebSocket to the backend gateway.
      // SvelteKit +server.ts routes cannot handle WS upgrades, so we bypass
      // the BFF and connect directly from the dev server.
      "/api/realtime": {
        target: "ws://localhost:8080",
        ws: true,
      },
    },
  },
});
