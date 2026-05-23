#!/usr/bin/env node
/*
 * tokens.json → tokens.css / tokens.d.ts generator.
 * Bespoke (zero-dep) by design — our token count is ~80, web-only, no Figma sync.
 * ESCALATE to Style Dictionary + @tokens-studio/sd-transforms ONLY when one of:
 *   (1) Figma → tokens sync is on the roadmap
 *   (2) Native iOS Swift / Android XML output is needed
 *   (3) Token count exceeds ~200
 * The tokens.json schema is the migration boundary; nothing else changes.
 *
 * Usage:
 *   node scripts/build-tokens.mjs             # regenerate tokens.css + tokens.d.ts
 *   node scripts/build-tokens.mjs --check     # verify generated output matches disk (CI parity test)
 */

import { readFileSync, writeFileSync } from 'fs';
import { join } from 'path';

const ROOT     = new URL('..', import.meta.url).pathname;
const SRC      = join(ROOT, 'packages/ui/tokens/tokens.json');
const OUT_CSS  = join(ROOT, 'packages/ui/src/lib/tokens.css');
const OUT_DTS  = join(ROOT, 'packages/ui/tokens/tokens.d.ts');
const CHECK    = process.argv.includes('--check');

const data = JSON.parse(readFileSync(SRC, 'utf8'));

// ── CSS generation ────────────────────────────────────────────────────────────

function indent(str, n) {
  return str.split('\n').map(l => ' '.repeat(n) + l).join('\n');
}

function renderTokenMap(tokens) {
  const lines = [];
  for (const [key, val] of Object.entries(tokens)) {
    if (key.startsWith('//')) {
      // Inline comment
      if (val === null) lines.push(`  /* ${key.slice(3)} */`);
      continue;
    }
    lines.push(`  ${key}: ${val};`);
  }
  return lines.join('\n');
}

function renderBlock(block) {
  const parts = [];
  if (block.comment) {
    const commentLines = block.comment.split('\n');
    if (commentLines.length === 1) {
      parts.push(`/* ── ${block.comment} ─────────────────────────────────────────────── */`);
    } else {
      parts.push(`/* ── ${commentLines[0]}`);
      for (let i = 1; i < commentLines.length; i++) parts.push(`   ${commentLines[i]}`);
      parts.push(`   ─────────────────────────────────────────────────────────────── */`);
    }
  }

  parts.push(`${block.selector} {`);

  if (block.groups) {
    const groupLines = [];
    for (const g of block.groups) {
      groupLines.push(`  /* ${g.comment} */`);
      groupLines.push(renderTokenMap(g.tokens));
    }
    parts.push(groupLines.join('\n'));
  } else if (block.tokens) {
    parts.push(renderTokenMap(block.tokens));
  }

  parts.push('}');
  return parts.join('\n');
}

const sections = [];

for (const block of data.blocks) {
  sections.push(renderBlock(block));
}

for (const rule of data.atRules ?? []) {
  const lines = [];
  if (rule.comment) {
    lines.push(`/* ── ${rule.comment} ────────────────────────────────────────────── */`);
  }
  lines.push(`${rule.rule} {`);
  lines.push(rule.body);
  lines.push('}');
  sections.push(lines.join('\n'));
}

const header = [
  `/* GENERATED — do not hand-edit.`,
  ` * Source of truth: packages/ui/tokens/tokens.json`,
  ` * Regenerate with: node scripts/build-tokens.mjs`,
  ` * Hand-editing tokens.css is forbidden after Phase 2.1a (2026-05-23).`,
  ` */`,
].join('\n');

const css = header + '\n\n' + sections.join('\n\n') + '\n';

// ── TypeScript declarations ────────────────────────────────────────────────────

const allTokens = [];
for (const block of data.blocks) {
  const collect = (tokens) => {
    for (const [key] of Object.entries(tokens)) {
      if (!key.startsWith('//')) allTokens.push(key);
    }
  };
  if (block.groups) {
    for (const g of block.groups) collect(g.tokens);
  } else if (block.tokens) {
    collect(block.tokens);
  }
}

const dtsLines = [
  `/** Auto-generated design token names. Run \`node scripts/build-tokens.mjs\` to regenerate. */`,
  `export type TokenName =`,
  ...allTokens.map((t, i) => `  | '${t}'${i === allTokens.length - 1 ? ';' : ''}`),
  ``,
  `export type SpacingToken   = '--space-1' | '--space-2' | '--space-3' | '--space-4' | '--space-5' | '--space-6' | '--space-7' | '--space-8';`,
  `export type RadiusToken    = '--radius-xs' | '--radius-sm' | '--radius-md' | '--radius-lg' | '--radius-xl' | '--radius-full';`,
  `export type DurationToken  = '--duration-fast' | '--duration-normal' | '--duration-stagger' | '--duration-slow' | '--duration-page';`,
  `export type ColorToken     = ${allTokens.filter(t => t.startsWith('--color-')).map(t => `'${t}'`).join(' | ')};`,
  `export type SpringToken    = '--spring-snappy' | '--spring-gentle' | '--spring-bouncy';`,
];
const dts = dtsLines.join('\n') + '\n';

// ── Output ─────────────────────────────────────────────────────────────────────

if (CHECK) {
  const existingCss = readFileSync(OUT_CSS, 'utf8');
  if (existingCss === css) {
    console.log('✅ build-tokens --check: tokens.css is up to date.');
    process.exit(0);
  } else {
    console.error('❌ build-tokens --check: tokens.css is out of date. Run `node scripts/build-tokens.mjs` to regenerate.');
    process.exit(1);
  }
}

writeFileSync(OUT_CSS, css);
writeFileSync(OUT_DTS, dts);
console.log(`Generated:\n  ${OUT_CSS.replace(ROOT, '')}\n  ${OUT_DTS.replace(ROOT, '')}`);
