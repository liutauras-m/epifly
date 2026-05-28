#!/usr/bin/env node
/**
 * CI gate: AUTH_AUTO_PROVISION_TENANTS=true must not appear in any
 * production compose / dokploy config file.
 *
 * Auto-provisioning is allowed only in dev/staging environments.
 * If it ships to prod, any user from any Zitadel org can create a
 * tenant on first login without operator approval — a security hole.
 *
 * Exit 1 (fail) if any violation is found.
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(fileURLToPath(import.meta.url), "..", "..");

/** Globs of file patterns that represent production configs */
const PROD_PATTERNS = [
  /dokploy[\\/]/,
  /docker-compose\.prod\./,
  /docker-compose\.production\./,
  /\.env\.prod(uction)?$/,
  /\.env\.prod\./,
];

/** The forbidden string */
const FORBIDDEN = "AUTH_AUTO_PROVISION_TENANTS=true";

const violations = [];

function walk(dir) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const entry of entries) {
    if (entry.name.startsWith(".") || entry.name === "node_modules") continue;
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(full);
    } else if (entry.isFile()) {
      const rel = relative(ROOT, full);
      if (!PROD_PATTERNS.some((p) => p.test(rel))) continue;
      let contents;
      try {
        contents = readFileSync(full, "utf8");
      } catch {
        continue;
      }
      const lines = contents.split("\n");
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        // Skip comments
        if (/^\s*#/.test(line)) continue;
        if (line.includes(FORBIDDEN)) {
          violations.push({ file: rel, line: i + 1, text: line.trim() });
        }
      }
    }
  }
}

walk(ROOT);

if (violations.length > 0) {
  console.error(
    `\n[assert-no-auto-provision-in-prod] FAIL: ${FORBIDDEN} found in prod config(s):\n`
  );
  for (const v of violations) {
    console.error(`  ${v.file}:${v.line}  →  ${v.text}`);
  }
  console.error(
    "\nAuto-provisioning must be disabled in production (AUTH_AUTO_PROVISION_TENANTS=false)."
  );
  console.error("Remove or comment out the violating lines, then re-run.\n");
  process.exit(1);
} else {
  console.log(`[assert-no-auto-provision-in-prod] OK — no violations found.`);
}
