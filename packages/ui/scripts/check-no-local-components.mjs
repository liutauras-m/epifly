#!/usr/bin/env node
/**
 * scripts/check-no-local-components.mjs — Phase 8.1 CI gate.
 *
 * Enforces Principle #7: no app-local UI components in apps/*.
 * All UI primitives must live in packages/ui — never defined inline in apps/.
 *
 * A violation is any .svelte file in apps/ that:
 *   1. Has a <style> block with > 20 significant lines (raw presentation logic)
 *   2. AND does not import from @conusai/ui (i.e., not wrapping a shared primitive)
 *
 * Allowed files:
 *   - Route files (+page.svelte, +layout.svelte, +error.svelte) — these are
 *     composition shells, not UI components. They may have thin styling (≤ 20 lines).
 *   - Files explicitly allowlisted below.
 *
 * Usage:
 *   node packages/ui/scripts/check-no-local-components.mjs
 *
 * Exit code: 0 = pass, 1 = violations found.
 */

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, extname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../../..', import.meta.url));
const APPS_DIR = join(ROOT, 'apps');

// Route/layout files are allowed — they are composition, not UI definition
const ROUTE_FILES = new Set([
  '+page.svelte',
  '+layout.svelte',
  '+error.svelte',
  '+page.server.ts',
  '+layout.server.ts',
]);

// Files explicitly excluded from checking (Phase 3 migration in-progress)
// Remove entries once the corresponding Phase 3/4 work lands.
const ALLOWLIST = new Set([
  'apps/browser-shell/src/lib/mobile/MobileShell.svelte',          // Phase 3.1 — pending migration
  'apps/browser-shell/src/lib/mobile/parts/ProfileSheet.svelte',   // Phase 3.2 — pending migration
  'apps/browser-shell/src/lib/mobile/parts/AttachmentSheet.svelte',// Phase 3.5 — pending migration
  'apps/browser-shell/src/lib/mobile/parts/WorkspaceCreateMenu.svelte', // Phase 3.2
  'apps/browser-shell/src/lib/mobile/parts/DrawerWorkspaceTree.svelte', // Phase 3.4
  'apps/browser-shell/src/lib/mobile/parts/DrawerProfileHeader.svelte', // Phase 3.4
  'apps/browser-shell/src/lib/mobile/parts/Breadcrumbs.svelte',    // Phase 3.3
  'apps/browser-shell/src/lib/mobile/parts/WorkspaceTreeRow.svelte',// Phase 3.4
]);

const STYLE_BLOCK_RE = /<style[^>]*>([\s\S]*?)<\/style>/g;
const SIGNIFICANT_LINE_RE = /^\s*[^/*\s{}.][^{};]{2,}/; // Rough: non-comment, non-selector lines with actual properties

const MAX_STYLE_LINES = 20;

function countSignificantStyleLines(styleBlock) {
  return styleBlock
    .split('\n')
    .filter(l => SIGNIFICANT_LINE_RE.test(l))
    .length;
}

function walkDir(dir, files = []) {
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === '.svelte-kit' || entry === 'dist') continue;
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) walkDir(full, files);
    else if (extname(entry) === '.svelte') files.push(full);
  }
  return files;
}

const violations = [];

for (const file of walkDir(APPS_DIR)) {
  const name = basename(file);
  if (ROUTE_FILES.has(name)) continue;

  const rel = relative(ROOT, file).replace(/\\/g, '/');
  if (ALLOWLIST.has(rel)) continue;

  const content = readFileSync(file, 'utf-8');

  // Count style lines
  let totalStyleLines = 0;
  STYLE_BLOCK_RE.lastIndex = 0;
  let m;
  while ((m = STYLE_BLOCK_RE.exec(content)) !== null) {
    totalStyleLines += countSignificantStyleLines(m[1]);
  }

  if (totalStyleLines > MAX_STYLE_LINES) {
    violations.push({ file: rel, styleLines: totalStyleLines });
  }
}

if (violations.length === 0) {
  console.log(`✓ check-no-local-components: no app-local UI components found`);
  process.exit(0);
} else {
  console.error(`✖ check-no-local-components: ${violations.length} app-local UI component(s) found`);
  console.error(`  Rule (Principle #7): non-route .svelte files in apps/ must not define`);
  console.error(`  > ${MAX_STYLE_LINES} significant style lines. Move UI to packages/ui.\n`);
  for (const v of violations) {
    console.error(`  ${v.file}  (${v.styleLines} significant style lines)`);
  }
  process.exit(1);
}
