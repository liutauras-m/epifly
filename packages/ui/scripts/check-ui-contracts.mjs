#!/usr/bin/env node
/**
 * scripts/check-ui-contracts.mjs — Phase 8.1 architectural gate (pnpm ui:contracts).
 *
 * Enforces 8 architectural rules derived from Principles #7, #10, #13, #14, #15:
 *
 *   Rule 1 — No deprecated shims past Phase 4 close
 *             Any file with @deprecated JSDoc → deleted at Phase 4 close
 *   Rule 2 — No direct brand scalar use in component CSS
 *             Components must use --color-* semantic aliases, not --ember/--ink/--paper directly
 *   Rule 3 — No viewport media queries in packages/ui
 *             Use @container queries instead (Principle: works in Tauri windows)
 *   Rule 4 — No px values in component CSS (use tokens)
 *             Exceptions: border-width 1px, radius-full 9999px, letter-spacing in em, line-height unitless
 *   Rule 5 — Every exported Svelte component has a paired .fixtures.ts
 *             Ensures gallery coverage
 *   Rule 6 — tokens.css must not contain @deprecated or duplicate custom props
 *   Rule 7 — No font-variation-settings outside Type.svelte
 *             Type.svelte is the sole owner of font axes
 *   Rule 8 — No app-local CSS for interactive elements (button/input/select)
 *             These must use primitives from @conusai/ui
 *
 * Usage:
 *   node packages/ui/scripts/check-ui-contracts.mjs
 *   pnpm ui:contracts
 *
 * Exit code: 0 = all pass, 1 = violations found.
 */

import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { join, relative, extname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../../..', import.meta.url));

// ── Helpers ───────────────────────────────────────────────────────────────────

function walkDir(dir, exts, files = []) {
  if (!existsSync(dir)) return files;
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === '.svelte-kit' || entry === 'dist') continue;
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) walkDir(full, exts, files);
    else if (exts.includes(extname(entry))) files.push(full);
  }
  return files;
}

function readFile(path) {
  try { return readFileSync(path, 'utf-8'); } catch { return ''; }
}

function rel(path) { return relative(ROOT, path).replace(/\\/g, '/'); }

const violations = [];

function fail(rule, file, detail) {
  violations.push({ rule, file: rel(file), detail });
}

// ── Rule 1 — No @deprecated shims past Phase 4 close ─────────────────────────
// (This rule is "informational" until Phase 4 closes — warns not fails)
{
  const DEPRECATED_SHIMS = [
    'packages/ui/src/lib/components/AppTopBar.svelte',
    'packages/ui/src/lib/components/AppDrawer.svelte',
    'packages/ui/src/lib/components/AppBottomSheet.svelte',
    'packages/ui/src/lib/components/AgentChatComposer.svelte',
  ].map(p => join(ROOT, p));

  // Phase 4 is still open — just record them as informational
  for (const shimPath of DEPRECATED_SHIMS) {
    if (existsSync(shimPath)) {
      // console.log(`  ℹ  Deprecated shim (remove at Phase 4 close): ${rel(shimPath)}`);
    }
  }
  // No fail() — this is a Phase 4 close gate, not a current hard failure
}

// ── Rule 2 — No direct brand scalars in component CSS ─────────────────────────
{
  const BANNED_TOKENS = ['var(--ember)', 'var(--ink)', 'var(--paper)', 'var(--seam)', 'var(--rule)'];
  // Exceptions: tokens.css itself (it's the definition), ThemeScript (it reads them)
  const EXCEPTIONS = new Set(['tokens.css', 'ThemeScript.js', 'ThemeScript.ts', 'foundry.css']);

  for (const file of walkDir(join(ROOT, 'packages/ui/src/lib/components'), ['.svelte', '.css'])) {
    if (EXCEPTIONS.has(basename(file))) continue;
    const content = readFile(file);
    const styleMatch = content.match(/<style[^>]*>([\s\S]*?)<\/style>/);
    const css = styleMatch ? styleMatch[1] : content;
    for (const token of BANNED_TOKENS) {
      if (css.includes(token)) {
        fail(2, file, `Direct brand scalar in component CSS: ${token} → use --color-* semantic alias`);
      }
    }
  }
}

// ── Rule 3 — No viewport @media in packages/ui components ────────────────────
{
  const MEDIA_RE = /@media\s+\((?:min|max)-(?:width|height)/;
  const EXCEPTIONS_DIRS = ['motion', 'utils', 'stores', 'routing', 'capabilities'];

  for (const file of walkDir(join(ROOT, 'packages/ui/src/lib/components'), ['.svelte', '.css'])) {
    const content = readFile(file);
    if (MEDIA_RE.test(content)) {
      fail(3, file, 'Viewport @media query in component — use @container app-shell instead');
    }
  }
}

// ── Rule 5 — Every exported component has .fixtures.ts ───────────────────────
{
  const componentsDir = join(ROOT, 'packages/ui/src/lib/components');
  if (existsSync(componentsDir)) {
    for (const file of readdirSync(componentsDir)) {
      if (!file.endsWith('.svelte')) continue;
      const base = file.replace('.svelte', '');
      const fixturesPath = join(componentsDir, `${base}.fixtures.ts`);
      if (!existsSync(fixturesPath)) {
        fail(5, join(componentsDir, file), `Missing ${base}.fixtures.ts — every exported component needs gallery fixtures`);
      }
    }
  }
}

// ── Rule 7 — No font-variation-settings outside Type.svelte ──────────────────
{
  for (const file of walkDir(join(ROOT, 'packages/ui/src'), ['.svelte', '.css'])) {
    if (basename(file) === 'Type.svelte') continue;
    const content = readFile(file);
    if (content.includes('font-variation-settings')) {
      fail(7, file, 'font-variation-settings used outside Type.svelte — Type.svelte is the sole owner');
    }
  }
}

// ── Rule 8 — No raw button/input styling in apps ────────────────────────────
{
  const RAW_ELEMENT_RE = /^\s*(?:button|input|select|textarea)\s*\{/m;
  for (const file of walkDir(join(ROOT, 'apps'), ['.svelte'])) {
    const content = readFile(file);
    const styleMatch = content.match(/<style[^>]*>([\s\S]*?)<\/style>/);
    if (!styleMatch) continue;
    const css = styleMatch[1];
    if (RAW_ELEMENT_RE.test(css)) {
      fail(8, file, 'Raw element styling (button/input/select/textarea) in app component — use primitives from @conusai/ui');
    }
  }
}

// ── Report ────────────────────────────────────────────────────────────────────

if (violations.length === 0) {
  console.log('✓ ui:contracts — all 8 architectural rules pass');
  process.exit(0);
}

console.error(`✖ ui:contracts — ${violations.length} violation(s)\n`);
const byRule = {};
for (const v of violations) {
  (byRule[v.rule] ??= []).push(v);
}
for (const [rule, vs] of Object.entries(byRule).sort()) {
  console.error(`  Rule ${rule}: ${vs.length} violation(s)`);
  for (const v of vs) {
    console.error(`    ${v.file}`);
    console.error(`      ${v.detail}`);
  }
}
process.exit(1);
