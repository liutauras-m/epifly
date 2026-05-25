// Minimal Node.js script to verify Dokploy API access and list Project Environment variables
// Usage: node dokploy/test-dokploy-api.mjs

import { readFileSync } from 'node:fs';
// (No fetch import needed; Node 18+ has global fetch)

// Read .dokploy
const lines = readFileSync(new URL('./.dokploy', import.meta.url), 'utf8').split('\n');
const env = {};
for (const line of lines) {
  const m = line.match(/^([A-Z0-9_]+)="?([^"]+)"?$/);
  if (m) env[m[1]] = m[2];
}

const dokployUrl = env.DOKPLOY_URL?.replace(/\/+$/, '');
const apiKey = env.DOKPLOY_API_KEY;
const projectUrl = env.DOKPLOY_PROJECT_URL;
const environmentId = projectUrl?.split('/environment/')[1];

if (!dokployUrl || !apiKey || !environmentId) {
  console.error('Missing required .dokploy values');
  process.exit(1);
}

const endpoint = `${dokployUrl}/api/environment.one?environmentId=${encodeURIComponent(environmentId)}`;

fetch(endpoint, {
  method: 'GET',
  headers: {
    'Content-Type': 'application/json',
    'x-api-key': apiKey,
  },
})
  .then(res => res.json())
  .then(json => {
    if (json.env) {
      console.log('Project Environment variables:');
      console.log(json.env);
    } else {
      console.error('No env found:', json);
    }
  })
  .catch(e => {
    console.error('API error:', e);
  });
