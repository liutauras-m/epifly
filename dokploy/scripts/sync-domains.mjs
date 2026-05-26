#!/usr/bin/env node
/**
 * Sync Dokploy compose-app domains from `dokploy/domains.yaml`.
 *
 * Runs in two modes:
 *
 *   1. Locally (operator): reads creds from `dokploy/.dokploy` if present.
 *      `APP_DOMAIN=… node dokploy/scripts/sync-domains.mjs [--dry-run] [--app NAME]`
 *
 *   2. Inside a Dokploy `domain-sync` init container on every deploy:
 *      All creds come from environment variables (Dokploy Shared Env):
 *        APP_DOMAIN, DOKPLOY_URL, DOKPLOY_ENVIRONMENT_ID, DOKPLOY_API_KEY
 *      `DOKPLOY_APP_NAME=infra node /app/scripts/sync-domains.mjs --app infra`
 *      (The compose mounts the whole dokploy/ dir at /app so the relative
 *      `lib/dokploy-client.mjs` import resolves.)
 *
 * Flags:
 *   --dry-run      Print plan, don't mutate Dokploy
 *   --app NAME     Only sync the app whose name (or appName) matches NAME.
 *                  Also accepts $DOKPLOY_APP_NAME from env.
 *
 * Zero npm dependencies — uses the bundled minimal YAML parser below, which
 * handles exactly the schema in `domains.yaml`.
 */

import { readFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const DOKPLOY_DIR = resolve(__dirname, "..");

const { makeClient } = await import(
  pathToFileURL(resolve(DOKPLOY_DIR, "lib/dokploy-client.mjs")).href
);

// ── CLI ────────────────────────────────────────────────────────────────────
const argv = process.argv.slice(2);
const DRY_RUN = argv.includes("--dry-run");
const TRIGGER_REDEPLOY =
  argv.includes("--trigger-redeploy") ||
  process.env.DOKPLOY_TRIGGER_REDEPLOY === "true";
const APP_FILTER =
  (() => {
    const i = argv.indexOf("--app");
    return i >= 0 ? argv[i + 1] : undefined;
  })() ?? process.env.DOKPLOY_APP_NAME;

// ── Credential resolution ──────────────────────────────────────────────────
// Env vars win. Fall back to `dokploy/.dokploy` for local runs.
function loadCreds() {
  const creds = {
    url: process.env.DOKPLOY_URL,
    apiKey: process.env.DOKPLOY_API_KEY,
    envId: process.env.DOKPLOY_ENVIRONMENT_ID,
  };
  const file = join(DOKPLOY_DIR, ".dokploy");
  if ((!creds.url || !creds.apiKey || !creds.envId) && existsSync(file)) {
    for (const line of readFileSync(file, "utf8").split("\n")) {
      const m = line.match(/^\s*([A-Z_]+)\s*=\s*"?([^"\n]*?)"?\s*$/);
      if (!m) continue;
      const [, k, v] = m;
      if (k === "DOKPLOY_URL" && !creds.url) creds.url = v;
      else if (k === "DOKPLOY_API_KEY" && !creds.apiKey) creds.apiKey = v;
      else if (k === "DOKPLOY_PROJECT_URL" && !creds.envId) {
        const em = v.match(/\/environment\/([^/?#]+)/);
        if (em) creds.envId = em[1];
      }
    }
  }
  if (!creds.url) fail("DOKPLOY_URL is required");
  if (!creds.apiKey) fail("DOKPLOY_API_KEY is required");
  if (!creds.envId) fail("DOKPLOY_ENVIRONMENT_ID is required");
  creds.url = creds.url.replace(/\/+$/, "");
  return creds;
}

// ── Minimal YAML parser (subset matching domains.yaml) ─────────────────────
// Supports: top-level map, nested lists of maps, scalar values, `#` comments.
function parseYamlSubset(text) {
  const lines = text
    .split(/\r?\n/)
    .map((l, i) => {
      const hash = l.indexOf("#");
      return { line: (hash >= 0 ? l.slice(0, hash) : l).replace(/\s+$/, ""), n: i + 1 };
    })
    .filter((x) => x.line.trim() !== "");

  let idx = 0;
  const indentOf = (l) => l.match(/^ */)[0].length;
  const scalar = (s) => {
    s = s.trim();
    if (s === "" || s === "~" || s === "null") return null;
    if (s === "true") return true;
    if (s === "false") return false;
    if (/^-?\d+$/.test(s)) return Number(s);
    if (/^-?\d*\.\d+$/.test(s)) return Number(s);
    if ((s.startsWith('"') && s.endsWith('"')) || (s.startsWith("'") && s.endsWith("'"))) {
      return s.slice(1, -1);
    }
    return s;
  };

  function parseBlock(minIndent) {
    if (idx >= lines.length || indentOf(lines[idx].line) < minIndent) return null;
    return lines[idx].line.slice(minIndent).startsWith("- ")
      ? parseList(minIndent)
      : parseMap(minIndent);
  }

  function parseMap(curIndent) {
    const out = {};
    while (idx < lines.length) {
      const { line, n } = lines[idx];
      const ind = indentOf(line);
      if (ind < curIndent) break;
      if (ind > curIndent) fail(`unexpected indent at line ${n}: "${line}"`);
      const body = line.slice(curIndent);
      const colon = body.indexOf(":");
      if (colon < 0) fail(`expected key:value at line ${n}: "${line}"`);
      const key = body.slice(0, colon).trim();
      const rest = body.slice(colon + 1);
      idx++;
      if (rest.trim() === "") {
        if (idx < lines.length && indentOf(lines[idx].line) > curIndent) {
          out[key] = parseBlock(indentOf(lines[idx].line));
        } else {
          out[key] = null;
        }
      } else {
        out[key] = scalar(rest);
      }
    }
    return out;
  }

  function parseList(curIndent) {
    const out = [];
    while (idx < lines.length) {
      const { line, n } = lines[idx];
      const ind = indentOf(line);
      if (ind < curIndent) break;
      const body = line.slice(curIndent);
      if (!body.startsWith("- ")) break;
      const itemStart = curIndent + 2;
      const inline = line.slice(itemStart);
      idx++;
      if (inline.includes(":")) {
        // Map item — push back a synthesized first map line at itemStart indent.
        lines.splice(idx, 0, { line: " ".repeat(itemStart) + inline, n });
        out.push(parseMap(itemStart));
      } else if (inline.trim() === "") {
        if (idx < lines.length && indentOf(lines[idx].line) > curIndent) {
          out.push(parseBlock(indentOf(lines[idx].line)));
        } else {
          out.push(null);
        }
      } else {
        out.push(scalar(inline));
      }
    }
    return out;
  }

  return parseBlock(0);
}

// ── Domain helpers ─────────────────────────────────────────────────────────
function interpolate(value, vars) {
  return String(value).replace(/\$\{([A-Z_]+)\}/g, (_, k) => {
    if (!(k in vars)) fail(`Undefined placeholder \${${k}} in domains.yaml`);
    return vars[k];
  });
}

function expandDomains(spec, vars) {
  return spec.apps.map((app) => ({
    appName: app.appName,
    domains: (app.domains ?? []).map((d) => ({
      host: interpolate(d.host, vars),
      serviceName: String(d.serviceName),
      port: Number(d.port),
      https: d.https ?? true,
      certificateType: d.certificateType ?? "letsencrypt",
      path: d.path ?? "/",
    })),
  }));
}

const key = (d) => `${d.serviceName}::${d.host.toLowerCase()}`;
const drifted = (a, b) =>
  a.port !== b.port ||
  a.https !== b.https ||
  (a.certificateType ?? "none") !== b.certificateType ||
  (a.path ?? "/") !== b.path;

// ── Main ───────────────────────────────────────────────────────────────────
async function main() {
  const { url, apiKey, envId } = loadCreds();
  const client = makeClient(url, apiKey);

  const appDomain = process.env.APP_DOMAIN;
  if (!appDomain) fail("APP_DOMAIN env var is required");

  // Locate domains.yaml: alongside script for local runs; /domains.yaml for
  // the init-container bind mount; cwd as last resort.
  const candidates = [
    join(DOKPLOY_DIR, "domains.yaml"),
    "/domains.yaml",
    resolve(process.cwd(), "domains.yaml"),
  ];
  const yamlPath = candidates.find((p) => existsSync(p));
  if (!yamlPath) fail(`domains.yaml not found in ${candidates.join(", ")}`);

  const spec = parseYamlSubset(readFileSync(yamlPath, "utf8"));
  if (!spec?.apps?.length) fail(`${yamlPath} is empty or has no \`apps:\` list`);

  let wantedApps = expandDomains(spec, { APP_DOMAIN: appDomain });
  if (APP_FILTER) {
    wantedApps = wantedApps.filter((a) => a.appName === APP_FILTER);
    if (wantedApps.length === 0) {
      console.log(`No app named "${APP_FILTER}" in ${yamlPath} — nothing to do.`);
      return;
    }
  }

  console.log(`Dokploy:     ${url}`);
  console.log(`Environment: ${envId}`);
  console.log(`APP_DOMAIN:  ${appDomain}`);
  console.log(`Filter:      ${APP_FILTER ?? "(all apps)"}`);
  console.log(`Mode:        ${DRY_RUN ? "DRY RUN" : "APPLY"}`);
  console.log("");

  const search = await client.query("compose.search", {
    environmentId: envId,
    limit: 100,
    offset: 0,
  });
  const byName = new Map();
  for (const c of search.items ?? []) {
    byName.set(c.appName, c.composeId);
    byName.set(c.name, c.composeId);
  }

  let create = 0,
    update = 0,
    del = 0;

  for (const app of wantedApps) {
    const composeId = byName.get(app.appName);
    if (!composeId) {
      console.warn(`⚠️  ${app.appName}: no Compose app found — skipping`);
      continue;
    }
    const current = await client.query("domain.byComposeId", { composeId });
    const cur = new Map(current.map((d) => [key(d), d]));
    const want = new Map(app.domains.map((d) => [key(d), d]));

    console.log(`▼ ${app.appName}  (composeId=${composeId})`);
    let touched = 0;

    for (const [k, d] of want) {
      const existing = cur.get(k);
      if (!existing) {
        console.log(`  + create  ${d.serviceName} → https://${d.host}  :${d.port}`);
        if (!DRY_RUN) {
          await client.mutate("domain.create", {
            host: d.host,
            path: d.path,
            port: d.port,
            https: d.https,
            certificateType: d.certificateType,
            composeId,
            serviceName: d.serviceName,
            domainType: "compose",
          });
        }
        create++;
        touched++;
      } else if (drifted(existing, d)) {
        console.log(
          `  ~ update  ${d.serviceName} → https://${d.host}  (:${existing.port}→:${d.port})`,
        );
        if (!DRY_RUN) {
          await client.mutate("domain.update", {
            domainId: existing.domainId,
            host: d.host,
            path: d.path,
            port: d.port,
            https: d.https,
            certificateType: d.certificateType,
            serviceName: d.serviceName,
            domainType: "compose",
          });
        }
        update++;
        touched++;
      }
    }
    for (const [k, d] of cur) {
      if (!want.has(k)) {
        console.log(`  - delete  ${d.serviceName} → https://${d.host}`);
        if (!DRY_RUN) {
          await client.mutate("domain.delete", { domainId: d.domainId });
        }
        del++;
        touched++;
      }
    }
    if (touched === 0) console.log(`  ✓ in sync (${current.length} domains)`);
  }

  console.log("");
  console.log(
    `${DRY_RUN ? "Would" : "Did"}: +${create} create, ~${update} update, -${del} delete`,
  );

  // Self-redeploy: if anything changed and we're scoped to a single app,
  // trigger a fresh deploy so Dokploy regenerates Traefik labels with the
  // updated domain set. The next run of this script will find nothing to
  // do and won't loop. Designed for the in-stack `domain-sync` init service.
  if (
    TRIGGER_REDEPLOY &&
    !DRY_RUN &&
    APP_FILTER &&
    wantedApps.length === 1 &&
    create + update + del > 0
  ) {
    const composeId = byName.get(wantedApps[0].appName);
    if (composeId) {
      console.log("");
      console.log(`↻ Triggering self-redeploy of ${APP_FILTER} (composeId=${composeId})`);
      await client.mutate("compose.deploy", { composeId });
      console.log("  Redeploy queued. Traefik will pick up new labels on next start.");
    }
  }
}

function fail(msg) {
  console.error(`✗ ${msg}`);
  process.exit(1);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
