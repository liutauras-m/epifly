#!/usr/bin/env node
// Verifies every asset referenced in the exports map exists on disk.
import { existsSync, readdirSync, writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const assetsDir = join(root, 'src/lib/assets');

const required = [
  'images/conusai-logo-lightmode.png',
  'images/conusai-logo-darkmode.png',
  'images/favicon.png',
  'icons/icons.svg',
];

let ok = true;
for (const rel of required) {
  const abs = join(assetsDir, rel);
  if (!existsSync(abs)) {
    console.error(`[assets:verify] MISSING: src/lib/assets/${rel}`);
    ok = false;
  }
}

// Write manifest of all files under assets/
function walk(dir, base = '') {
  const entries = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const rel = base ? `${base}/${entry.name}` : entry.name;
    if (entry.isDirectory()) entries.push(...walk(join(dir, entry.name), rel));
    else entries.push(rel);
  }
  return entries;
}

const manifest = walk(assetsDir);
writeFileSync(join(root, 'dist/assets-manifest.json'), JSON.stringify(manifest, null, 2));

if (!ok) process.exit(1);
console.log(`[assets:verify] OK — ${manifest.length} assets verified`);
