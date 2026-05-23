#!/usr/bin/env node
/**
 * scripts/build-tokens.mjs
 * Regenerates tokens.css and tokens.d.ts from tokens/tokens.json.
 *
 * Usage: node scripts/build-tokens.mjs
 *
 * Called by pnpm build and by the "Source of truth" note in tokens.json.
 * Hand-editing tokens.css is forbidden after Phase 2.1a.
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join, dirname } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT      = join(__dirname, '..');

const src  = JSON.parse(readFileSync(join(ROOT, 'tokens/tokens.json'), 'utf-8'));

// ── CSS builder ──────────────────────────────────────────────────────────────

function tokensToCss(tokens) {
  const lines = [];
  for (const [k, v] of Object.entries(tokens)) {
    if (k.startsWith('//') || v === null) continue;
    if (k.startsWith('// ')) {
      lines.push(`  /* ${k.slice(3)} */`);
    } else {
      lines.push(`  ${k}: ${v};`);
    }
  }
  return lines.join('\n');
}

function blockToCss(block) {
  const sections = [];

  // Flat token map
  if (block.tokens) {
    const body = tokensToCss(block.tokens);
    if (body) sections.push(body);
  }

  // Grouped token maps
  if (block.groups) {
    for (const group of block.groups) {
      if (group.comment) sections.push(`  /* ${group.comment} */`);
      if (group.tokens) sections.push(tokensToCss(group.tokens));
    }
  }

  if (!sections.length) return '';
  return `/* ── ${block.comment} ${'─'.repeat(Math.max(0, 50 - block.comment.length))} */\n${block.selector} {\n${sections.join('\n')}\n}`;
}

const header = `/* GENERATED — do not hand-edit.
 * Source of truth: packages/ui/tokens/tokens.json
 * Regenerate with: node scripts/build-tokens.mjs
 * Hand-editing tokens.css is forbidden after Phase 2.1a (2026-05-23).
 */\n`;

const cssBlocks = src.blocks.map(blockToCss).filter(Boolean);
const atRules   = (src.atRules ?? []).map(r => {
  return `/* ── ${r.comment} ${'─'.repeat(Math.max(0, 50 - r.comment.length))} */\n${r.rule} {\n${r.body}\n}`;
});

const css = [header, ...cssBlocks, ...atRules].join('\n\n') + '\n';
writeFileSync(join(ROOT, 'src/lib/tokens.css'), css, 'utf-8');
console.log('✓ tokens.css written');

// ── TypeScript declaration builder ───────────────────────────────────────────

function collectTokenNames(tokens, out = new Set()) {
  for (const k of Object.keys(tokens)) {
    if (!k.startsWith('//') && k.startsWith('--')) out.add(k);
  }
  return out;
}

const allNames = new Set();
for (const block of src.blocks) {
  if (block.tokens) collectTokenNames(block.tokens, allNames);
  if (block.groups) {
    for (const g of block.groups) {
      if (g.tokens) collectTokenNames(g.tokens, allNames);
    }
  }
}

const dts = `// GENERATED — do not hand-edit.
// Source of truth: packages/ui/tokens/tokens.json
// Regenerate with: node scripts/build-tokens.mjs

export type FoundryToken =
${[...allNames].map(n => `  | '${n}'`).join('\n')};

declare module '*.css' {
  const content: Record<string, string>;
  export default content;
}
`;
writeFileSync(join(ROOT, 'tokens/tokens.d.ts'), dts, 'utf-8');
console.log('✓ tokens.d.ts written');
