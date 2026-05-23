#!/usr/bin/env node
/**
 * scripts/check-motion-purpose.mjs — Phase 6 motion purpose tag gate.
 *
 * Enforces Principle #14: every animation in component CSS must have a
 * purpose tag visible in code:
 *   - A comment matching / *(feedback|continuity|hierarchy|delight)/ within 5 lines
 *   - A data-motion-purpose="…" attribute on the same element
 *   - OR the file is in the motion/ helpers allowlist (tags live on keyframe defs)
 *
 * Currently runs as WARNING (exits 0) — flips to ERROR (exits 1) at Phase 7 close.
 * Set env var MOTION_PURPOSE_ENFORCE=1 to enable hard failure now.
 *
 * Usage:
 *   node packages/ui/scripts/check-motion-purpose.mjs
 *
 * Exit code: 0 = pass (or warnings only), 1 = hard failure when enforced.
 */

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT   = fileURLToPath(new URL('../../..', import.meta.url));
const ENFORCE = process.env.MOTION_PURPOSE_ENFORCE === '1';

// Files in these directories self-document purpose on the keyframe definitions
const ALLOWED_DIRS = [
  'packages/ui/src/lib/motion',
  'packages/ui/scripts',
];

const PURPOSE_TAG_RE = /\[(feedback|continuity|hierarchy|delight)\]/i;
const DATA_ATTR_RE   = /data-motion-purpose\s*=\s*["'](feedback|continuity|hierarchy|delight)/i;
const TRANSITION_RE  = /transition\s*:|animation\s*:/;
const SVELTE_DIR_RE  = /(?:transition:|in:|out:|use:animate)/;

const SCAN_EXTS = new Set(['.svelte', '.css']);

function walkDir(dir, files = []) {
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === '.svelte-kit' || entry === 'dist') continue;
    const full = join(dir, entry);
    const stat  = statSync(full);
    if (stat.isDirectory()) walkDir(full, files);
    else if (SCAN_EXTS.has(extname(entry))) files.push(full);
  }
  return files;
}

function isInAllowedDir(file) {
  const rel = relative(ROOT, file).replace(/\\/g, '/');
  return ALLOWED_DIRS.some(d => rel.startsWith(d));
}

function getWindowLines(lines, lineNum, radius = 5) {
  const start = Math.max(0, lineNum - radius);
  const end   = Math.min(lines.length - 1, lineNum + radius);
  return lines.slice(start, end + 1).join('\n');
}

const warnings = [];

for (const file of walkDir(join(ROOT, 'packages/ui/src'))) {
  if (isInAllowedDir(file)) continue;

  const content = readFileSync(file, 'utf-8');
  const lines   = content.split('\n');

  lines.forEach((line, i) => {
    if (!TRANSITION_RE.test(line) && !SVELTE_DIR_RE.test(line)) return;
    // Skip reduced-motion overrides and zero durations
    if (/0\.01ms|0ms|transition:\s*none|animation:\s*none/.test(line)) return;
    // Skip CSS variable declarations (those are just tokens, not live animations)
    if (/^\s*--/.test(line)) return;

    const window = getWindowLines(lines, i);
    if (PURPOSE_TAG_RE.test(window) || DATA_ATTR_RE.test(window)) return;

    warnings.push({
      file:    relative(ROOT, file),
      line:    i + 1,
      snippet: line.trim(),
    });
  });
}

if (warnings.length === 0) {
  console.log('✓ check-motion-purpose: all animations have purpose tags');
  process.exit(0);
}

const level = ENFORCE ? 'error' : 'warning';
console[level === 'error' ? 'error' : 'warn'](
  `${ENFORCE ? '✖' : '⚠'} check-motion-purpose: ${warnings.length} animation(s) missing purpose tag [feedback|continuity|hierarchy|delight]`
);
if (!ENFORCE) {
  console.warn(`  (Running as WARNING — set MOTION_PURPOSE_ENFORCE=1 to enforce at Phase 7 close)\n`);
}
for (const w of warnings) {
  console[level === 'error' ? 'error' : 'warn'](`  ${w.file}:${w.line}  "${w.snippet}"`);
}

process.exit(ENFORCE ? 1 : 0);
