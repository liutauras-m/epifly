#!/usr/bin/env node
/**
 * check-design-tokens.mjs  (Phase 1.2 + 2.1e CI gate)
 *
 * Fails with exit 1 when it finds:
 *   1. Raw hex colours  (#rgb / #rrggbb / #rrggbbaa) outside the token files.
 *   2. Hard-coded `px` values in layout / typography properties outside token files.
 *   3. `cubic-bezier(` or bare `transition: …Nms` outside motion token files.
 *   4. Any <style> block in apps/* containing colour, radius, or size literals
 *      (catches inline drift the other rules miss).
 *   5. (§2.1e) Short-form token aliases (--s-*, --r-*, --t-*, --dur-*, --rail,
 *      --font-body, --font-display) used via var() in consumer code. These were
 *      migrated to canonical long-form names in §2.1c; any new occurrence in a PR
 *      means someone used the compat alias instead of the canonical name.
 *      Currently warn-only; flips to error at Phase 4 close per ui-plan.md.
 *
 * TOKEN FILES (excluded from rules 1–4; rule 5 also excludes them):
 *   packages/ui/src/lib/tokens.css   (generated — defines the aliases)
 *   packages/ui/src/lib/foundry.css
 *   packages/ui/tokens/tokens.json
 *   packages/ui/tokens/tokens.d.ts
 *
 * Usage:
 *   node scripts/check-design-tokens.mjs            # check everything
 *   node scripts/check-design-tokens.mjs --warn     # print but do not exit 1
 */

import { readFileSync, readdirSync } from 'fs';
import { join, relative, extname } from 'path';

const ROOT   = new URL('..', import.meta.url).pathname;
const WARN   = process.argv.includes('--warn');

const TOKEN_FILES = new Set([
  'packages/ui/src/lib/tokens.css',
  'packages/ui/src/lib/foundry.css',
  'packages/ui/tokens/tokens.json',
  'packages/ui/tokens/tokens.d.ts',
  // codemod + changelog reference old names — exclude them
  'scripts/rename-token.mjs',
  'docs/ui-tokens-changelog.md',
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

// Exemption patterns — lines matching these are intentional and skip the raw-px check:
//   1. var(--token, Npx) — CSS variable fallback value is not a violation
//   2. @container/@media breakpoints — architectural pixel breakpoints are expected
//   3. CSS math: max(Npx,...), min(Npx,...), clamp(Npx,...) — fluid sizing expressions
//   4. 1px only (border-width, divider) — plan explicitly allows this
//   5. visually-hidden helper: width:1px; height:1px; padding:0 (LiveAnnouncer pattern)
const PX_EXEMPTIONS = [
  /var\([^)]*\d+px[^)]*\)/,                  // any var(...px...) fallback
  /@container\b.*\(\s*(min|max)-width\s*:/,  // container query
  /@media\b.*\(\s*(min|max)-width\s*:/,      // media query
  /\bclamp\([^)]*\d+px/,                     // clamp()
  /\bmin\([^)]*\d+px/,                       // min()
  /\bmax\([^)]*\d+px/,                       // max()
  /^\s*width\s*:\s*1px\s*;/,                 // visually-hidden 1px
  /^\s*height\s*:\s*1px\s*;/,                // visually-hidden 1px
  /border(?:-\w+)?\s*:\s*1px\b/,             // border/border-* 1px
  /outline(?:-width)?\s*:\s*1px\b/,          // outline 1px
  /padding\s*:\s*0\s*;/,                     // padding: 0 (zero has no unit)
  /font-size\s*:\s*0\s*;/,                   // font-size: 0
  /margin\s*:\s*-1px\s*;/,                   // sr-only visually-hidden -1px margin (WCAG clip technique)
];

// For apps/* style blocks: same checks applied only to the <style> content
function extractStyleBlocks(src) {
  return [...src.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map(m => m[1]);
}

// ── scanning ──────────────────────────────────────────────────────────────────

const errors = [];
const warnings = [];

// Scope-based severity rules:
//   components/ + .css  → hard error on everything
//   features/           → error on raw-hex; warn on raw-px (design-specific values expected)
//   apps/browser-shell/src/lib/mobile/ → warn-only (Phase 3 migration target)
//   apps/web/src/routes/ → error on raw-hex; warn on raw-px (legacy routes)
function getSeverity(filePath, rule) {
  if (
    filePath.includes('browser-shell/src/lib/mobile/') ||
    filePath.includes('browser-shell/src/lib/parts/')
  ) return 'warn';
  if (rule === 'raw-px') {
    if (filePath.includes('/features/') || filePath.includes('apps/web/src/routes/')) {
      return 'warn';
    }
  }
  return 'error';
}

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

    function push(rule, text) {
      const sev = getSeverity(path, rule);
      const entry = { file: path, line: lineno, rule, text };
      if (sev === 'error') errors.push(entry);
      else warnings.push(entry);
    }

    if (RAW_HEX.test(line)) {
      // Exempt: hex used as a fallback inside var(--token, #hex) — not a raw use
      const isHexFallback = /var\([^)]*,\s*(?:linear-gradient[^)]*)?#[0-9a-fA-F]{3,8}\b/.test(line)
        || /var\([^)]*#[0-9a-fA-F]{3,8}\b/.test(line);
      if (!isHexFallback) push('raw-hex', trimmed.slice(0, 120));
    }
    if (LAYOUT_PROPS.test(line)) {
      const isExempt = PX_EXEMPTIONS.some(re => re.test(line));
      if (!isExempt) push('raw-px', trimmed.slice(0, 120));
    }
    if (CUBIC_BEZIER.test(line)) {
      // Exempt: cubic-bezier used as a fallback inside var(--ease-xxx, cubic-bezier(...))
      const isCubicFallback = /var\([^)]*,\s*cubic-bezier\(/.test(line);
      if (!isCubicFallback) push('cubic-bezier', trimmed.slice(0, 120));
    }
    if (RAW_DURATION.test(line)) {
      // Exempt: duration used as a fallback inside var(--duration-xxx, Nms)
      const isDurationFallback = /var\([^)]*,\s*\d+ms\s*\)/.test(line);
      if (!isDurationFallback) push('raw-duration', trimmed.slice(0, 120));
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

// ── §2.1e short-form alias detection ─────────────────────────────────────────
// Detects var(--s-N), var(--r-*), var(--t-*), var(--dur-*), var(--sidebar),
// var(--font-family-sans), var(--font-family-sans) in consumer code.
// These were migrated to canonical names in §2.1c; warn-only until Phase 4.
const SHORT_FORM_PATTERN = /var\(--(s-[1-8]|r-(?:xs|sm|md|lg|xl|full)|t-(?:display|h1|h2|body|meta|label|mono)|dur-(?:1|2|2b|3|4)|rail|font-(?:body|display))\b\s*[,)]/;

const shortFormErrors = [];

for (const dir of ['packages/ui/src', 'apps/web/src', 'apps/browser-shell/src']) {
  for (const file of walk(join(ROOT, dir), ['.svelte', '.css', '.ts', '.tsx'])) {
    const path = rel(file);
    if (TOKEN_FILES.has(path)) continue;
    const src = readFileSync(file, 'utf8');
    const lines = src.split('\n');
    lines.forEach((line, i) => {
      if (SHORT_FORM_PATTERN.test(line)) {
        shortFormErrors.push({ file: path, line: i + 1, rule: 'short-form-alias', text: line.trim().slice(0, 120) });
      }
    });
  }
}

if (shortFormErrors.length > 0) {
  console.warn(`\nWARN (§2.1e): ${shortFormErrors.length} short-form token alias usage(s) found.`);
  console.warn('  Use canonical long-form names (--space-N, --radius-*, --font-size-*, --duration-*, --sidebar).');
  console.warn('  These will become CI errors at Phase 4 close.\n');
  for (const { file, line, text } of shortFormErrors.slice(0, 20)) {
    console.warn(`  ${file}:${line}  ${text}`);
  }
  if (shortFormErrors.length > 20) console.warn(`  … and ${shortFormErrors.length - 20} more.`);
}

// ── report ────────────────────────────────────────────────────────────────────

function reportGroup(label, items, logFn) {
  if (!items.length) return;
  const byRule = {};
  for (const e of items) (byRule[e.rule] ??= []).push(e);
  logFn(`\n${label}: check-design-tokens found ${items.length} violation(s):\n`);
  for (const [rule, ruleItems] of Object.entries(byRule)) {
    logFn(`  ── ${rule} (${ruleItems.length}) ──`);
    for (const { file, line, text } of ruleItems) {
      logFn(`    ${file}:${line}  ${text}`);
    }
  }
}

// Print warnings (features/, apps/browser-shell/mobile/, apps/web routes) — non-blocking
if (warnings.length > 0) {
  reportGroup('WARN', warnings, console.warn.bind(console));
  console.warn('\n  Warnings: features/ and browser-shell/mobile/ have raw px values scheduled for migration.');
  console.warn('  Use --warn flag or fix before Phase 4 close.\n');
}

if (errors.length === 0 && shortFormErrors.length === 0) {
  const warnMsg = warnings.length > 0 ? ` (${warnings.length} warning(s) in features/apps)` : '';
  console.log(`✅ check-design-tokens: no hard violations found${warnMsg}.`);
  process.exit(0);
}

const byRule = {};
for (const e of errors) {
  (byRule[e.rule] ??= []).push(e);
}

const label = WARN ? 'WARN' : 'ERROR';
console.error(`\n${label}: check-design-tokens found ${errors.length} hard violation(s):\n`);

for (const [rule, items] of Object.entries(byRule)) {
  console.error(`  ── ${rule} (${items.length}) ──`);
  for (const { file, line, text } of items) {
    console.error(`    ${file}:${line}  ${text}`);
  }
  console.error('');
}

console.error(`Run \`node scripts/check-design-tokens.mjs --warn\` to see violations without failing.\n`);
process.exit(WARN ? 0 : 1);
