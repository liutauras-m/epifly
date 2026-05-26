import { defineConfig } from "tsup";

export default defineConfig({
  entry: { epifly: "src/cli.ts" },
  format: ["esm"],
  outExtension: () => ({ js: ".mjs" }),
  target: "node22",
  bundle: true,
  splitting: false,
  sourcemap: true,
  clean: true,
  banner: {
    js: "#!/usr/bin/env node",
  },
  esbuildOptions(options) {
    // Preserve native node: imports (do not bundle Node builtins)
    options.external = [
      "node:child_process",
      "node:crypto",
      "node:fs",
      "node:http",
      "node:path",
      "node:url",
      "node:os",
      "node:process",
    ];
  },
});
