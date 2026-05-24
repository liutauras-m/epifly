#!/usr/bin/env node
/**
 * dump-ui-inventory.mjs
 *
 * Generates docs/ui-inventory.md with:
 *   1. Every component in packages/ui/src/lib/{components,features,capabilities}
 *      — runes status (uses $props / export let / mixed), line count
 *   2. Every route in apps/web/src/routes and apps/browser-shell/src/routes
 *   3. <style> block violations in apps/* (app-local CSS is a shared-UI violation)
 *
 * Usage:  node scripts/dump-ui-inventory.mjs
 *         node scripts/dump-ui-inventory.mjs --json   (also writes docs/ui-inventory.json)
 */

import { readFileSync, writeFileSync, readdirSync, statSync } from 'fs';
import { join, relative, extname } from 'path';

const ROOT = new URL('..', import.meta.url).pathname;
const OUT_MD   = join(ROOT, 'docs/ui-inventory.md');
const OUT_JSON = join(ROOT, 'docs/ui-inventory.json');
const emitJson = process.argv.includes('--json');

// ── helpers ──────────────────────────────────────────────────────────────────

function walk(dir, exts = ['.svelte', '.ts', '.js']) {
  const results = [];
  try {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) results.push(...walk(full, exts));
      else if (exts.includes(extname(entry.name))) results.push(full);
    }
  } catch { /* dir doesn't exist */ }
  return results;
}

function rel(p) { return relative(ROOT, p); }

function runesStatus(src) {
  const hasRunesDirective = /svelte:options\s+runes=\{true\}/.test(src);
  const hasProps  = /\$props\s*\(/.test(src);
  const hasState  = /\$state\s*[(<]/.test(src);
  const hasEffect = /\$effect\s*\(/.test(src);
  const hasExport = /export\s+let\s+/.test(src);
  const hasDollar = /\$:/.test(src);
  // Explicit runes opt-in covers components that use context/stores but not $props()
  if ((hasRunesDirective || hasProps || hasState || hasEffect) && !hasExport) return '✅ runes';
  if (!hasProps && !hasRunesDirective && (hasExport || hasDollar)) return '⚠️  legacy';
  if (hasProps && hasExport) return '🔀 mixed';
  return '—';
}

function styleViolations(src) {
  // Look for raw hex, px in layout props, or cubic-bezier inside <style> blocks
  const styleBlocks = [...src.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)]
    .map(m => m[1]);
  const hits = [];
  for (const block of styleBlocks) {
    if (/#[0-9a-fA-F]{3,8}\b/.test(block))           hits.push('raw-hex');
    if (/(?:padding|margin|gap|font-size|line-height|border-radius)\s*:[^;]*\d+px/.test(block)) hits.push('raw-px');
    if (/cubic-bezier\(/.test(block))                  hits.push('cubic-bezier');
    if (/transition\s*:[^;]*\d+ms/.test(block))        hits.push('raw-ms');
  }
  return [...new Set(hits)];
}

// ── 1. UI package components ──────────────────────────────────────────────────

const UI_BASE = join(ROOT, 'packages/ui/src/lib');
const uiDirs = ['components', 'features', 'capabilities'];

const components = [];
for (const dir of uiDirs) {
  for (const file of walk(join(UI_BASE, dir))) {
    if (!file.endsWith('.svelte')) continue;
    const src = readFileSync(file, 'utf8');
    const lines = src.split('\n').length;
    components.push({
      dir,
      path: rel(file),
      name: file.split('/').pop().replace('.svelte', ''),
      lines,
      runes: runesStatus(src),
    });
  }
}

const runesCount    = components.filter(c => c.runes === '✅ runes').length;
const legacyCount   = components.filter(c => c.runes === '⚠️  legacy').length;
const mixedCount    = components.filter(c => c.runes === '🔀 mixed').length;

// ── 2. Routes ─────────────────────────────────────────────────────────────────

function collectRoutes(routesDir, appName) {
  const rows = [];
  for (const file of walk(routesDir, ['.svelte'])) {
    if (!file.includes('+page') && !file.includes('+error') && !file.includes('+layout')) continue;
    const src = readFileSync(file, 'utf8');
    // Infer route URL from file path
    const raw = relative(routesDir, file);
    let rel_ = raw
      .replace(/\/?(\+page|\+error|\+layout)\.svelte$/, '')
      || '/';
    const special = raw.match(/\+(page|error|layout)/)?.[1];
    if (special && special !== 'page') rel_ += ` [${special}]`;
    if (!rel_.startsWith('/')) rel_ = '/' + rel_;
    const sharedImports = [...src.matchAll(/from\s+['"]@conusai\/ui['"]/g)].length
      + [...src.matchAll(/from\s+['"]@conusai\/ui\//g)].length;
    const appLocalStyles = /<style/.test(src);
    rows.push({ app: appName, route: rel_, path: rel(file), sharedImports, appLocalStyles });
  }
  return rows;
}

const routes = [
  ...collectRoutes(join(ROOT, 'apps/web/src/routes'), 'web'),
  ...collectRoutes(join(ROOT, 'apps/browser-shell/src/routes'), 'browser-shell'),
];

// ── 3. Style violations in apps/* ────────────────────────────────────────────

const appViolations = [];
for (const appDir of ['apps/web/src', 'apps/browser-shell/src']) {
  for (const file of walk(join(ROOT, appDir), ['.svelte'])) {
    const src = readFileSync(file, 'utf8');
    if (!/<style/.test(src)) continue;
    const viols = styleViolations(src);
    appViolations.push({ path: rel(file), violations: viols.length > 0 ? viols : ['<style> block present'] });
  }
}

// ── build markdown ────────────────────────────────────────────────────────────

const now = new Date().toISOString().slice(0, 10);

let md = `# UI Component Inventory

> Auto-generated by \`scripts/dump-ui-inventory.mjs\` on ${now}.
> Re-run with \`node scripts/dump-ui-inventory.mjs\` to refresh.

## Summary

| Stat | Count |
|------|-------|
| Total Svelte components (packages/ui) | ${components.length} |
| ✅ Runes | ${runesCount} |
| ⚠️  Legacy (export let / \`$:\`) | ${legacyCount} |
| 🔀 Mixed | ${mixedCount} |
| Routes | ${routes.length} |
| App-local \`<style>\` blocks (violations) | ${appViolations.length} |

---

## Components

`;

for (const dir of uiDirs) {
  const group = components.filter(c => c.dir === dir);
  if (group.length === 0) continue;
  md += `### \`${dir}/\`\n\n`;
  md += `| Component | Lines | Runes |\n|-----------|-------|-------|\n`;
  for (const c of group) {
    md += `| [\`${c.name}\`](../${c.path}) | ${c.lines} | ${c.runes} |\n`;
  }
  md += '\n';
}

md += `---

## Routes

| App | Route | File | Shared imports | App-local styles? |
|-----|-------|------|---------------|-------------------|
`;
for (const r of routes) {
  md += `| ${r.app} | \`${r.route}\` | [\`${r.path.split('/').pop()}\`](../${r.path}) | ${r.sharedImports} | ${r.appLocalStyles ? '⚠️  yes' : '✅ no'} |\n`;
}

md += `
---

## App-local \`<style>\` violations

These files have \`<style>\` blocks in \`apps/*\`. They are violation candidates for the shared-UI rule (all styling should live in \`packages/ui\`). Count is tracked as a regression metric.

`;

if (appViolations.length === 0) {
  md += '_None — clean!_\n';
} else {
  md += `| File | Issues |\n|------|--------|\n`;
  for (const v of appViolations) {
    md += `| [\`${v.path}\`](../${v.path}) | ${v.violations.join(', ')} |\n`;
  }
}

md += `\n---\n_End of inventory._\n`;

writeFileSync(OUT_MD, md);
console.log(`Written: ${rel(OUT_MD)} (${components.length} components, ${routes.length} routes, ${appViolations.length} style violations)`);

if (emitJson) {
  writeFileSync(OUT_JSON, JSON.stringify({ generated: now, components, routes, appViolations }, null, 2));
  console.log(`Written: ${rel(OUT_JSON)}`);
}
