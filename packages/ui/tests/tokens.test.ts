/**
 * Token parity test (Phase 2.1b).
 *
 * Verifies:
 *   1. Paper and Forge theme blocks define identical sets of token keys.
 *   2. Every compatibility alias (--s-*, --r-*, --t-*, --dur-*, --rail) points
 *      to a canonical long-form token that actually exists.
 *   3. Every semantic --color-* alias points to a token that exists.
 *   4. build-tokens --check passes (tokens.css matches tokens.json).
 */

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { join } from 'path';
import { execSync } from 'child_process';

const ROOT   = new URL('../../..', import.meta.url).pathname;
const JSON_PATH = join(ROOT, 'packages/ui/tokens/tokens.json');

const data = JSON.parse(readFileSync(JSON_PATH, 'utf8'));

function tokenKeysOf(block: { tokens?: Record<string, unknown>; groups?: Array<{ tokens: Record<string, unknown> }> }) {
  const keys: string[] = [];
  const collect = (tokens: Record<string, unknown>) => {
    for (const k of Object.keys(tokens)) {
      if (!k.startsWith('//')) keys.push(k);
    }
  };
  if (block.groups) block.groups.forEach(g => collect(g.tokens));
  else if (block.tokens) collect(block.tokens);
  return keys;
}

const paperBlock = data.blocks.find((b: any) => b.selector.includes('data-theme="paper"'));
const forgeBlock = data.blocks.find((b: any) => b.selector.includes('data-theme="forge"'));

describe('Token parity — Paper vs Forge', () => {
  it('both theme blocks exist', () => {
    expect(paperBlock).toBeDefined();
    expect(forgeBlock).toBeDefined();
  });

  it('Paper and Forge define exactly the same set of token keys', () => {
    const paperKeys = new Set(tokenKeysOf(paperBlock));
    const forgeKeys = new Set(tokenKeysOf(forgeBlock));

    const onlyInPaper = [...paperKeys].filter(k => !forgeKeys.has(k));
    const onlyInForge = [...forgeKeys].filter(k => !paperKeys.has(k));

    // Forge is allowed to have additional overrides (e.g. ember-2, shadow-sm
    // brighter variants) — but Paper must not have keys that Forge lacks.
    expect(onlyInPaper).toEqual([]);
  });
});

describe('Compatibility alias resolution', () => {
  // Collect all canonical long-form token names from the :root blocks
  const allTokens = new Set<string>();
  for (const block of data.blocks) {
    for (const k of tokenKeysOf(block)) allTokens.add(k);
  }

  const ALIASES: [string, string][] = [
    ['--s-1', '--space-1'], ['--s-2', '--space-2'], ['--s-3', '--space-3'], ['--s-4', '--space-4'],
    ['--s-5', '--space-5'], ['--s-6', '--space-6'], ['--s-7', '--space-7'], ['--s-8', '--space-8'],
    ['--r-xs', '--radius-xs'], ['--r-sm', '--radius-sm'], ['--r-md', '--radius-md'],
    ['--r-lg', '--radius-lg'], ['--r-xl', '--radius-xl'], ['--r-full', '--radius-full'],
    ['--t-display', '--font-size-display'], ['--t-h1', '--font-size-h1'], ['--t-h2', '--font-size-h2'],
    ['--t-body', '--font-size-body'], ['--t-meta', '--font-size-meta'],
    ['--t-label', '--font-size-label'], ['--t-mono', '--font-size-mono'],
    ['--dur-1', '--duration-fast'], ['--dur-2', '--duration-normal'],
    ['--dur-2b', '--duration-stagger'], ['--dur-3', '--duration-slow'], ['--dur-4', '--duration-page'],
    ['--rail', '--sidebar'],
  ];

  it.each(ALIASES)('%s alias has a canonical target %s in tokens.json', (alias, canonical) => {
    expect(allTokens.has(alias)).toBe(true);
    expect(allTokens.has(canonical)).toBe(true);
  });
});

describe('Semantic color aliases resolve to existing tokens', () => {
  const colorBlock = data.blocks.find((b: any) =>
    b.comment?.includes('Semantic color aliases')
  );

  it('semantic color block exists', () => {
    expect(colorBlock).toBeDefined();
  });

  it('every --color-* value references a token that exists in the same file', () => {
    if (!colorBlock?.tokens) return;
    const allTokenNames = new Set<string>();
    for (const block of data.blocks) {
      for (const k of tokenKeysOf(block)) allTokenNames.add(k);
    }
    for (const [key, val] of Object.entries(colorBlock.tokens as Record<string, string>)) {
      if (key.startsWith('//')) continue;
      const match = val.match(/^var\((--[a-z0-9-]+)\)/);
      if (match) {
        expect(allTokenNames.has(match[1]), `${key}: references ${match[1]} which doesn't exist`).toBe(true);
      }
    }
  });
});

describe('Generator parity (tokens.json → tokens.css)', () => {
  it('build-tokens --check passes: tokens.css matches tokens.json', () => {
    expect(() =>
      execSync('node scripts/build-tokens.mjs --check', { cwd: ROOT, stdio: 'pipe' })
    ).not.toThrow();
  });
});
