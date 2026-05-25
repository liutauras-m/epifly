// Minimal Node.js script to verify Dokploy API access and list Project Environment variables
// Usage: node dokploy/test-dokploy-api.mjs
//
// Dokploy exposes its API over tRPC at `/api/trpc/<procedure>` — the previous
// `/api/environment.one?environmentId=…` shape always 404'd because there is
// no such REST endpoint. We call the tRPC procedure the same way
// scripts/sync-domains.mjs and epifly-deploy/scripts/deploy.mjs do, then
// resolve the projectId from the environment record and read project.env
// (Shared Env lives on the project, not the environment).

import { readFileSync } from 'node:fs';
// (No fetch import needed; Node 18+ has global fetch)

// Read .dokploy
const lines = readFileSync(new URL('./.dokploy', import.meta.url), 'utf8').split('\n');
const env = {};
for (const line of lines) {
  const m = line.match(/^\s*([A-Z0-9_]+)\s*=\s*"?([^"\n]*?)"?\s*$/);
  if (m) env[m[1]] = m[2];
}

const dokployUrl = env.DOKPLOY_URL?.replace(/\/+$/, '');
const apiKey = env.DOKPLOY_API_KEY;
const projectUrl = env.DOKPLOY_PROJECT_URL;
const environmentId = projectUrl?.split('/environment/')[1]?.split(/[/?#]/)[0];

if (!dokployUrl || !apiKey || !environmentId) {
  console.error('Missing required .dokploy values (DOKPLOY_URL, DOKPLOY_API_KEY, DOKPLOY_PROJECT_URL)');
  process.exit(1);
}

async function trpcQuery(procedure, input) {
  const url = `${dokployUrl}/api/trpc/${procedure}?input=${encodeURIComponent(
    JSON.stringify({ json: input }),
  )}`;
  const res = await fetch(url, {
    method: 'GET',
    headers: { 'x-api-key': apiKey, accept: 'application/json' },
  });
  const text = await res.text();
  let body = null;
  try { body = text ? JSON.parse(text) : null; } catch { /* ignore */ }
  if (!res.ok) {
    const msg = body?.error?.json?.message ?? body?.message ?? text;
    throw new Error(`${procedure} → HTTP ${res.status}: ${msg}`);
  }
  return body?.result?.data?.json ?? body?.result?.data ?? body;
}

try {
  const envRec = await trpcQuery('environment.one', { environmentId });
  const projectId = envRec?.projectId ?? envRec?.project?.projectId;
  if (!projectId) {
    console.error('environment.one returned no projectId:', envRec);
    process.exit(1);
  }
  const project = await trpcQuery('project.one', { projectId });
  if (project?.env) {
    console.log('Project Environment variables:');
    console.log(project.env);
  } else {
    console.error('No env found on project:', project);
  }
} catch (e) {
  console.error('API error:', e.message ?? e);
  process.exit(1);
}
