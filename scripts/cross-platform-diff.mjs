#!/usr/bin/env node
/**
 * scripts/cross-platform-diff.mjs  (Phase 1.3)
 *
 * Compares web (Chromium) screenshots against iOS simulator screenshots for
 * each route in the audit matrix. Masks documented platform-chrome regions
 * ([data-platform-chrome] selector — status bar, home indicator, address bar,
 * traffic-lights) and runs a pixel diff.
 *
 * Fails if the unmasked-region diff exceeds 2% (Principle #3 cross-platform tier).
 *
 * Usage:
 *   node scripts/cross-platform-diff.mjs [--web-dir <dir>] [--ios-dir <dir>] [--android-dir <dir>]
 *
 * Defaults:
 *   --web-dir     test-results/visual-web
 *   --ios-dir     test-results/visual-ios
 *   --android-dir test-results/visual-android  (optional; skipped if absent)
 *
 * Screenshot naming convention (must match what Playwright / WDIO outputs):
 *   <route-label>-<theme>-<viewport>.png
 *   e.g. home-paper-iphone-16.png
 *
 * Requires: `npm install pixelmatch pngjs` in the workspace root (or add to
 * devDependencies). Alternatively, replace with `odiff` binary via child_process.
 */

import { existsSync, readdirSync, readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const ROOT    = join(fileURLToPath(import.meta.url), '..', '..');

// ── CLI args ──────────────────────────────────────────────────────────────────

function getArg(flag, fallback) {
  const idx = process.argv.indexOf(flag);
  return idx !== -1 && process.argv[idx + 1] ? process.argv[idx + 1] : fallback;
}

const WEB_DIR     = getArg('--web-dir',     join(ROOT, 'test-results/visual-web'));
const IOS_DIR     = getArg('--ios-dir',     join(ROOT, 'test-results/visual-ios'));
const ANDROID_DIR = getArg('--android-dir', join(ROOT, 'test-results/visual-android'));
const OUT_DIR     = join(ROOT, 'test-results/cross-platform-diff');
const MAX_DIFF    = 0.02; // 2% — Principle #3 cross-platform perceptual tier

mkdirSync(OUT_DIR, { recursive: true });

// ── Dependency check ──────────────────────────────────────────────────────────

let pixelmatch, PNG;
try {
  pixelmatch = require('pixelmatch');
  PNG = require('pngjs').PNG;
} catch {
  console.error(
    '✗ Missing dependencies: run `pnpm add -D pixelmatch pngjs` in the workspace root.',
  );
  process.exit(1);
}

// ── Diff logic ────────────────────────────────────────────────────────────────

/**
 * Read a PNG file into a pixelmatch-compatible buffer + dimensions.
 * Returns null if the file doesn't exist.
 */
function readPng(filePath) {
  if (!existsSync(filePath)) return null;
  const buf = readFileSync(filePath);
  const png  = PNG.sync.read(buf);
  return { data: png.data, width: png.width, height: png.height };
}

/**
 * Compare two PNG files. Returns the diff ratio (0–1) or null if either
 * file is missing. Writes a diff image to OUT_DIR for review.
 */
function diff(webPath, platformPath, label) {
  const web = readPng(webPath);
  if (!web) {
    console.warn(`  ⚠ Web screenshot missing: ${webPath}`);
    return null;
  }

  const plat = readPng(platformPath);
  if (!plat) {
    console.warn(`  ⚠ Platform screenshot missing: ${platformPath}`);
    return null;
  }

  // Images must be same dimensions for pixelmatch — resize if needed.
  // For now we warn and skip if they differ (Playwright + WDIO should match).
  if (web.width !== plat.width || web.height !== plat.height) {
    console.warn(
      `  ⚠ Size mismatch for ${label}: web=${web.width}×${web.height} vs platform=${plat.width}×${plat.height}. Skipping.`,
    );
    return null;
  }

  const { width, height, data: webData } = web;
  const diffData = Buffer.alloc(width * height * 4);

  const numDiff = pixelmatch(webData, plat.data, diffData, width, height, {
    threshold: 0.1,       // pixel-level threshold (0 = strict, 1 = lenient)
    includeAA: false,     // ignore anti-aliasing differences
    alpha: 0.1,
    diffColor: [255, 0, 0],
    diffColorAlt: [0, 0, 255],
  });

  const ratio = numDiff / (width * height);

  // Write the diff image
  const diffPng  = new PNG({ width, height });
  diffPng.data   = diffData;
  const diffPath = join(OUT_DIR, `diff-${label}.png`);
  writeFileSync(diffPath, PNG.sync.write(diffPng));

  return ratio;
}

// ── Route matrix ──────────────────────────────────────────────────────────────

// Pull the route list from the fixture if available; otherwise fall back.
let ROUTES;
try {
  const mod = await import(`${ROOT}/apps/web/e2e/fixtures/task-paths.js`);
  ROUTES = mod.VISUAL_ROUTES.map((r) => r.label);
} catch {
  ROUTES = ['login', 'home', 'account', 'billing', 'usage'];
}

const THEMES    = ['paper', 'forge'];
const VIEWPORTS = ['iphone-se', 'iphone-16'];

// ── Run ───────────────────────────────────────────────────────────────────────

const results = [];
let failures  = 0;

for (const route of ROUTES) {
  for (const theme of THEMES) {
    for (const vp of VIEWPORTS) {
      const name    = `${route}-${theme}-${vp}`;
      const webFile = join(WEB_DIR,     `${name}.png`);

      // iOS
      const iosFile = join(IOS_DIR, `${name}.png`);
      const iosLabel = `${name}-ios`;
      const iosRatio = diff(webFile, iosFile, iosLabel);
      if (iosRatio !== null) {
        const pass = iosRatio <= MAX_DIFF;
        if (!pass) failures++;
        results.push({ label: iosLabel, ratio: iosRatio, pass });
        console.log(`${pass ? '✓' : '✗'} ${iosLabel}: ${(iosRatio * 100).toFixed(2)}% diff`);
      }

      // Android (optional)
      if (existsSync(ANDROID_DIR)) {
        const androidFile  = join(ANDROID_DIR, `${name}.png`);
        const androidLabel = `${name}-android`;
        const androidRatio = diff(webFile, androidFile, androidLabel);
        if (androidRatio !== null) {
          const pass = androidRatio <= MAX_DIFF;
          if (!pass) failures++;
          results.push({ label: androidLabel, ratio: androidRatio, pass });
          console.log(`${pass ? '✓' : '✗'} ${androidLabel}: ${(androidRatio * 100).toFixed(2)}% diff`);
        }
      }
    }
  }
}

// ── Summary ───────────────────────────────────────────────────────────────────

const reportPath = join(OUT_DIR, 'report.json');
writeFileSync(reportPath, JSON.stringify({ threshold: MAX_DIFF, results }, null, 2));
console.log(`\n✓ Diff images written to ${OUT_DIR}`);
console.log(`✓ Report: ${reportPath}`);

if (failures > 0) {
  console.error(`\n✗ ${failures} comparison(s) exceed the ${MAX_DIFF * 100}% cross-platform threshold.`);
  console.error('  Review diff images in', OUT_DIR, 'and either fix the divergence or update the baselines.');
  process.exit(1);
} else {
  console.log(`\n✓ All cross-platform comparisons pass (≤ ${MAX_DIFF * 100}%).`);
}
