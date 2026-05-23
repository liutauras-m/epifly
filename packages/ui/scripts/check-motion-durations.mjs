#!/usr/bin/env node
/**
 * scripts/check-motion-durations.mjs — Phase 6 motion audit gate.
 *
 * Enforces: no single transition-duration or animation-duration in CSS / Svelte
 * files exceeds 400 ms, except allowlisted cascade animations (page-load hierarchy).
 *
 * Usage:
 *   node packages/ui/scripts/check-motion-durations.mjs
 *
 * Exit code: 0 = pass, 1 = violations found.
 */

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../../..', import.meta.url));

// Cascade allowlist — these ids / names are [hierarchy] tagged and intentional
const ALLOWLIST = new Set([
  '--duration-page',    // 520ms — page transition token
  'cascade-in',         // keyframe for load cascade
  'view-fade-in',       // view transition fallback
]);

// File extensions to scan
const SCAN_EXTS = new Set(['.svelte', '.css', '.ts', '.js']);

// Regex: matches `NNNms` values in transition/animation declarations
const DURATION_RE = /(\d+(?:\.\d+)?)\s*ms/g;
const TRANSITION_LINE_RE = /(?:transition|animation)(?:-duration)?[^;{}]{0,120}/g;
const SKIP_RE = /(?:animation-duration|transition-duration)\s*:\s*0/;

const MAX_MS = 400;

function walkDir(dir, files = []) {
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === '.svelte-kit' || entry === 'dist') continue;
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) walkDir(full, files);
    else if (SCAN_EXTS.has(extname(entry))) files.push(full);
  }
  return files;
}

function isAllowlisted(line) {
  for (const term of ALLOWLIST) {
    if (line.includes(term)) return true;
  }
  return false;
}

const violations = [];

for (const file of walkDir(join(ROOT, 'packages/ui/src'))) {
  const content = readFileSync(file, 'utf-8');
  const lines = content.split('\n');
  lines.forEach((line, i) => {
    // Skip reduced-motion overrides and zero-duration resets
    if (SKIP_RE.test(line)) return;
    if (isAllowlisted(line)) return;

    const lineNum = i + 1;
    let m;
    // Reset lastIndex
    TRANSITION_LINE_RE.lastIndex = 0;
    while ((m = TRANSITION_LINE_RE.exec(line)) !== null) {
      const fragment = m[0];
      DURATION_RE.lastIndex = 0;
      let dm;
      while ((dm = DURATION_RE.exec(fragment)) !== null) {
        const ms = parseFloat(dm[1]);
        if (ms > MAX_MS) {
          violations.push({ file: relative(ROOT, file), line: lineNum, ms, fragment: fragment.trim() });
        }
      }
    }
  });
}

if (violations.length === 0) {
  console.log(`✓ check-motion-durations: all animation durations ≤ ${MAX_MS}ms`);
  process.exit(0);
} else {
  console.error(`✖ check-motion-durations: ${violations.length} violation(s) found`);
  console.error(`  Rule: no single animation/transition duration may exceed ${MAX_MS}ms`);
  console.error(`  Exception: allowlisted cascade animations (--duration-page, cascade-in, view-fade-in)\n`);
  for (const v of violations) {
    console.error(`  ${v.file}:${v.line}  →  ${v.ms}ms  in: "${v.fragment}"`);
  }
  process.exit(1);
}
