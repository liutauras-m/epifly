#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';

const repoRoot = process.cwd();
const routesMod = path.join(repoRoot, 'apps/backend/crates/agent-gateway/src/routes/mod.rs');
const mainRs = path.join(repoRoot, 'apps/backend/crates/agent-gateway/src/main.rs');

function read(file) {
  return fs.readFileSync(file, 'utf8');
}

function normalizeMethod(method) {
  const m = method.toLowerCase();
  if (m === 'get') return 'GET';
  if (m === 'post') return 'POST';
  if (m === 'put') return 'PUT';
  if (m === 'patch') return 'PATCH';
  if (m === 'delete') return 'DELETE';
  if (m === 'options') return 'OPTIONS';
  return method.toUpperCase();
}

function extractRouteTable(source) {
  const out = [];
  const re = /RouteEntry\s*\{[\s\S]*?method:\s*"([A-Z]+)"[\s\S]*?path:\s*"([^"]+)"[\s\S]*?\}/g;
  let match = re.exec(source);
  while (match) {
    out.push({ method: match[1], path: match[2] });
    match = re.exec(source);
  }
  return out;
}

function extractWiredRoutes(source) {
  const out = [];
  const re = /\.route\(\s*"([^"]+)"\s*,\s*(?:[a-z_]+::)*([a-z_]+)\s*\(/g;
  let match = re.exec(source);
  while (match) {
    out.push({ method: normalizeMethod(match[2]), path: match[1] });
    match = re.exec(source);
  }

  // Swagger UI mount contributes documented GET routes without explicit .route() calls.
  const swaggerRe = /SwaggerUi::new\(\s*"([^"]+)"\s*\)\.url\(\s*"([^"]+)"/g;
  let swaggerMatch = swaggerRe.exec(source);
  while (swaggerMatch) {
    out.push({ method: 'GET', path: swaggerMatch[1] });
    out.push({ method: 'GET', path: swaggerMatch[2] });
    swaggerMatch = swaggerRe.exec(source);
  }

  return out;
}

function key(r) {
  return `${r.method} ${r.path}`;
}

function main() {
  const modSrc = read(routesMod);
  const mainSrc = read(mainRs);

  const table = extractRouteTable(modSrc);
  const wired = [...extractWiredRoutes(modSrc), ...extractWiredRoutes(mainSrc)];

  const tableSet = new Set(table.map(key));
  const wiredSet = new Set(wired.map(key));

  const missingInWiring = [...tableSet].filter((k) => !wiredSet.has(k));
  const undocumentedInTable = [...wiredSet].filter((k) => !tableSet.has(k));

  if (missingInWiring.length === 0 && undocumentedInTable.length === 0) {
    console.log('OK: ROUTE_TABLE and router wiring are in sync');
    return;
  }

  if (missingInWiring.length > 0) {
    console.error('ERROR: ROUTE_TABLE entries missing from router wiring:');
    for (const entry of missingInWiring) {
      console.error(`  - ${entry}`);
    }
  }

  if (undocumentedInTable.length > 0) {
    console.error('ERROR: Wired routes missing from ROUTE_TABLE:');
    for (const entry of undocumentedInTable) {
      console.error(`  - ${entry}`);
    }
  }

  process.exit(1);
}

main();
