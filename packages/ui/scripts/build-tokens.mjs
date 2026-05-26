#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const rootScript = join(here, "../../../scripts/build-tokens.mjs");
const result = spawnSync(process.execPath, [rootScript], { stdio: "inherit" });

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}
