#!/usr/bin/env node
// Asserts structural parity between the TS CapabilityCard and the Rust schema.
// Hashes the JSON-Schema shape of CapabilityCard from openapi.d.ts against the
// canonical domain.ts definition and fails if they diverge.
import { createHash } from 'crypto';
import { readFileSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const openapiPath = join(root, 'src/openapi.d.ts');

if (!existsSync(openapiPath)) {
  console.warn('[assert-parity] openapi.d.ts not yet generated — skipping parity check.');
  process.exit(0);
}

const openapi = readFileSync(openapiPath, 'utf8');

// Extract the CapabilityCard interface block from the generated file
const match = openapi.match(/CapabilityCard\s*[=:]\s*\{([^}]+)\}/);
if (!match) {
  console.warn('[assert-parity] CapabilityCard not found in openapi.d.ts — skipping.');
  process.exit(0);
}

const rustFields = match[1]
  .split('\n')
  .map(l => l.trim())
  .filter(l => l && !l.startsWith('//'))
  .sort()
  .join('\n');

const rustHash = createHash('sha256').update(rustFields).digest('hex').slice(0, 12);

// Extract from domain.ts
const domain = readFileSync(join(root, 'src/domain.ts'), 'utf8');
const domainMatch = domain.match(/interface CapabilityCard\s*\{([^}]+)\}/);
if (!domainMatch) {
  console.error('[assert-parity] CapabilityCard not found in domain.ts');
  process.exit(1);
}

const tsFields = domainMatch[1]
  .split('\n')
  .map(l => l.trim())
  .filter(l => l && !l.startsWith('//'))
  .sort()
  .join('\n');

const tsHash = createHash('sha256').update(tsFields).digest('hex').slice(0, 12);

if (rustHash !== tsHash) {
  console.error(`[assert-parity] CapabilityCard MISMATCH`);
  console.error(`  Rust (openapi.d.ts): ${rustHash}`);
  console.error(`  TS   (domain.ts):    ${tsHash}`);
  console.error('Update domain.ts to match the generated openapi.d.ts schema.');
  process.exit(1);
}

console.log(`[assert-parity] CapabilityCard OK — hash ${tsHash}`);
