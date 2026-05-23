#!/usr/bin/env node
/**
 * check-design-tokens.mjs  (Phase 1.2 CI gate)
 *
 * Fails with exit 1 when it finds:
 *   1. Raw hex colours  (#rgb / #rrggbb / #rrggbbaa) outside the token files.
 *   2. Hard-coded `px` values in layout / typography properties outside token files.
 *   3. `cubic-bezier(` or bare `transition: …Nms` outside motion token files.
 *   4. Any <style> block in apps/* containing colour, radius, or size literals
 *      (catches inline drift the other rules miss).
 *
 * TOKEN FILES (excluded from all checks):
 *   packages/ui/src/lib/tokens.css
 *   packages/ui/src/lib/foundry.css
 *
 * Usage:
 *   node scripts/check-design-tokens.mjs            # check everything
 *   node scripts/check-design-tokens.mjs --warn     # print but do not exit 1
 */

import { readFileSync, readdirSync, statSync } from 'fs';
import { join, relative, extname } from 'path';

const ROOT   = new URL('..', import.meta.url).pathname;
const WARN   = process.argv.includes('--warn');

const TOKEN_FILES = new Set([
  'packages/ui/src/lib/tokens.css',
  'packages/ui/src/lib/foundry.css',
]);

// ── file walker ───────────────────────────────────────────────────────────────

function walk(dir, exts) {
  const out = [];
  let entries;
  try { entries = readdirSync(dir, { withFileTypes: true }); }
  catch { return out; }
  for (const e of entries) {
    if (e.name === 'node_modules' || e.name === '.svelte-kit' || e.name === 'dist' || e.name === 'build' || e.name === '.git') continue;
    const full = join(dir, e.name);
    if (e.isDirectory()) out.push(...walk(full, exts));
    else if (exts.includes(extname(e.name))) out.push(full);
  }
  return out;
}

function rel(p) { return relative(ROOT, p); }

// ── rules ─────────────────────────────────────────────────────────────────────

const LAYOUT_PROPS = /(?:padding|margin|gap|font-size|font-weight|line-height|border-radius|width|height|min-width|max-width|min-height|max-height)\s*:[^;{]*\d+px/;
const RAW_HEX      = /#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/;
const CUBIC_BEZIER = /cubic-bezier\(/;
const RAW_DURATION = /transition\s*:[^;]*\d+ms/;

// For apps/* style blocks: same checks applied only to the <style> content
function extractStyleBlocks(src) {
  return [...src.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map(m => m[1]);
}

// ── scanning ──────────────────────────────────────────────────────────────────

const errors = [];

function check(file, src, inStyleBlock = false) {
  const path = rel(file);
  if (TOKEN_FILES.has(path)) return;

  const lines = src.split('\n');
  lines.forEach((line, i) => {
    const lineno = i + 1;
    const trimmed = line.trim();
    // Skip comments
    if (/^\s*(\/\/|\/\*|\*)/.test(line)) return;
    if (trimmed.startsWith('*') || trimmed.startsWith('//')) return;

    if (RAW_HEX.test(line)) {
      errors.push({ file: path, line: lineno, rule: 'raw-hex', text: trimmed.slice(0, 120) });
    }
    if (LAYOUT_PROPS.test(line)) {
      errors.push({ file: path, line: lineno, rule: 'raw-px', text: trimmed.slice(0, 120) });
    }
    if (CUBIC_BEZIER.test(line)) {
      errors.push({ file: path, line: lineno, rule: 'cubic-bezier', text: trimmed.slice(0, 120) });
    }
    if (RAW_DURATION.test(line)) {
      errors.push({ file: path, line: lineno, rule: 'raw-duration', text: trimmed.slice(0, 120) });
    }
  });
}

// Check all CSS files (excluding token files)
for (const file of walk(join(ROOT, 'packages/ui/src/lib'), ['.css'])) {
  check(file, readFileSync(file, 'utf8'));
}

// Check Svelte component files in packages/ui (their <style> blocks)
for (const file of walk(join(ROOT, 'packages/ui/src/lib'), ['.svelte'])) {
  const src = readFileSync(file, 'utf8');
  const blocks = extractStyleBlocks(src);
  for (const block of blocks) {
    check(file, block);
  }
}

// Check apps/* Svelte files — any <style> block with raw values is a violation
for (const appDir of ['apps/web/src', 'apps/browser-shell/src']) {
  for (const file of walk(join(ROOT, appDir), ['.svelte'])) {
    const src = readFileSync(file, 'utf8');
    const blocks = extractStyleBlocks(src);
    if (blocks.length === 0) continue;
    for (const block of blocks) {
      check(file, block);
    }
  }
}

// ── report ────────────────────────────────────────────────────────────────────

if (errors.length === 0) {
  console.log('✅ check-design-tokens: no violations found.');
  process.exit(0);
}

const byRule = {};
for (const e of errors) {
  (byRule[e.rule] ??= []).push(e);
}

const label = WARN ? 'WARN' : 'ERROR';
console.error(`\n${label}: check-design-tokens found ${errors.length} violation(s):\n`);

for (const [rule, items] of Object.entries(byRule)) {
  console.error(`  ── ${rule} (${items.length}) ──`);
  for (const { file, line, text } of items) {
    console.error(`    ${file}:${line}  ${text}`);
  }
  console.error('');
}

console.error(`Run \`node scripts/check-design-tokens.mjs --warn\` to see violations without failing.\n`);
process.exit(WARN ? 0 : 1);
