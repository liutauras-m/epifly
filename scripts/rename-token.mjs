#!/usr/bin/env node
/**
 * rename-token.mjs  (Phase 2.1c codemod helper)
 *
 * Renames a CSS custom property across the entire repo:
 *   - Updates packages/ui/tokens/tokens.json
 *   - Regenerates packages/ui/src/lib/tokens.css
 *   - Rewrites every var(--old) → var(--new) in .css / .svelte / .ts / .tsx
 *   - Appends a one-line entry to docs/ui-tokens-changelog.md
 *
 * Usage:
 *   node scripts/rename-token.mjs --from --old-name --to --new-name [--pr 123]
 *   node scripts/rename-token.mjs --short-to-long   # batch: all --s-N → --space-N etc.
 *   node scripts/rename-token.mjs --dry-run --from --old-name --to --new-name
 */

import { readFileSync, writeFileSync, readdirSync, statSync } from 'fs';
import { join, extname } from 'path';
import { execSync } from 'child_process';

const ROOT      = new URL('..', import.meta.url).pathname;
const TOKENS_JSON = join(ROOT, 'packages/ui/tokens/tokens.json');
const CHANGELOG   = join(ROOT, 'docs/ui-tokens-changelog.md');

const args = process.argv.slice(2);
const DRY  = args.includes('--dry-run');
const BATCH_SHORT_TO_LONG = args.includes('--short-to-long');

// ── file walker ──────────────────────────────────────────────────────────────

const TARGET_EXTS = new Set(['.css', '.svelte', '.ts', '.tsx', '.js', '.mjs']);
const SKIP_DIRS   = new Set(['node_modules', '.svelte-kit', 'dist', 'build', '.git', 'target']);

function walk(dir) {
  const out = [];
  let entries;
  try { entries = readdirSync(dir, { withFileTypes: true }); }
  catch { return out; }
  for (const e of entries) {
    if (SKIP_DIRS.has(e.name)) continue;
    const full = join(dir, e.name);
    if (e.isDirectory()) out.push(...walk(full));
    else if (TARGET_EXTS.has(extname(e.name))) out.push(full);
  }
  return out;
}

// ── rename logic ─────────────────────────────────────────────────────────────

function renameInFile(filePath, oldName, newName) {
  const src = readFileSync(filePath, 'utf8');
  // Replace var(--old) and also bare --old: occurrences (token definitions)
  const next = src
    .replace(new RegExp(`var\\(${escapeRe(oldName)}\\)`, 'g'), `var(${newName})`)
    .replace(new RegExp(`(?<=[\\s;{,])${escapeRe(oldName)}(?=\\s*:)`, 'g'), newName);
  if (next !== src) {
    if (!DRY) writeFileSync(filePath, next);
    return true;
  }
  return false;
}

function escapeRe(s) { return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'); }

function renameInJson(oldName, newName) {
  const src = readFileSync(TOKENS_JSON, 'utf8');
  const next = src.replaceAll(`"${oldName}"`, `"${newName}"`);
  if (next !== src && !DRY) writeFileSync(TOKENS_JSON, next);
  return next !== src;
}

function appendChangelog(oldName, newName, pr) {
  const date = new Date().toISOString().slice(0, 10);
  const line = `| ${date} | \`${oldName}\` | \`${newName}\` | ${pr ? `[#${pr}](https://github.com/conusai/platform/pull/${pr})` : '—'} |\n`;
  let content;
  try {
    content = readFileSync(CHANGELOG, 'utf8');
  } catch {
    content = `# Token rename changelog\n\n| Date | Old name | New name | PR |\n|------|----------|----------|----|n`;
  }
  if (!DRY) writeFileSync(CHANGELOG, content + line);
}

function doRename(oldName, newName, pr) {
  if (!oldName.startsWith('--') || !newName.startsWith('--')) {
    console.error('Token names must start with --');
    process.exit(1);
  }
  console.log(`${DRY ? '[dry-run] ' : ''}Renaming ${oldName} → ${newName}`);

  let fileCount = 0;
  for (const file of walk(ROOT)) {
    if (renameInFile(file, oldName, newName)) fileCount++;
  }

  const jsonChanged = renameInJson(oldName, newName);

  if (!DRY) {
    // Regenerate tokens.css after JSON update
    if (jsonChanged) {
      execSync('node scripts/build-tokens.mjs', { cwd: ROOT, stdio: 'inherit' });
    }
    appendChangelog(oldName, newName, pr);
  }

  console.log(`  ${fileCount} file(s) updated${DRY ? ' (dry-run)' : ''}.`);
}

// ── batch: --short-to-long ────────────────────────────────────────────────────

const SHORT_TO_LONG = [
  ['--s-1', '--space-1'], ['--s-2', '--space-2'], ['--s-3', '--space-3'], ['--s-4', '--space-4'],
  ['--s-5', '--space-5'], ['--s-6', '--space-6'], ['--s-7', '--space-7'], ['--s-8', '--space-8'],
  ['--r-xs', '--radius-xs'], ['--r-sm', '--radius-sm'], ['--r-md', '--radius-md'],
  ['--r-lg', '--radius-lg'], ['--r-xl', '--radius-xl'], ['--r-full', '--radius-full'],
  ['--t-display', '--font-size-display'], ['--t-h1', '--font-size-h1'], ['--t-h2', '--font-size-h2'],
  ['--t-body', '--font-size-body'], ['--t-meta', '--font-size-meta'],
  ['--t-label', '--font-size-label'], ['--t-mono', '--font-size-mono'],
  ['--dur-1', '--duration-fast'], ['--dur-2', '--duration-normal'],
  ['--dur-2b', '--duration-stagger'], ['--dur-3', '--duration-slow'], ['--dur-4', '--duration-page'],
  ['--font-display', '--font-family-sans'], ['--font-body', '--font-family-sans'],
  ['--rail', '--sidebar'],
];

if (BATCH_SHORT_TO_LONG) {
  console.log(`Batch rename: short-to-long (${SHORT_TO_LONG.length} pairs)${DRY ? ' [dry-run]' : ''}`);
  for (const [old, nw] of SHORT_TO_LONG) doRename(old, nw, null);
  process.exit(0);
}

// ── single rename ─────────────────────────────────────────────────────────────

const fromIdx = args.indexOf('--from');
const toIdx   = args.indexOf('--to');
const prIdx   = args.indexOf('--pr');

if (fromIdx === -1 || toIdx === -1) {
  console.log(`Usage:
  node scripts/rename-token.mjs --from --old-name --to --new-name [--pr 123] [--dry-run]
  node scripts/rename-token.mjs --short-to-long [--dry-run]`);
  process.exit(1);
}

const oldName = args[fromIdx + 1];
const newName = args[toIdx + 1];
const pr      = prIdx !== -1 ? args[prIdx + 1] : null;

doRename(oldName, newName, pr);
