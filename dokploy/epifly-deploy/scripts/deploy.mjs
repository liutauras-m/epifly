#!/usr/bin/env node
/**
 * epifly-deploy — Dokploy project orchestrator.
 *
 * Runs once per deploy of the `epifly-deploy` compose service. Reconciles the
 * entire Epifly project against this repo's declarative configuration:
 *
 *   Phase 0  ensureVolumes()      Pre-create `external: true` named Docker
 *                                 volumes via the Engine API (data-safe)
 *   Phase 1  ensureSharedEnv()    Generate any missing secrets, PUT environment.env
 *   Phase 2  ensureComposes()     Create infra/gateway/web/observability/capabilities
 *                                 composes if missing; copy git source from self;
 *                                 wire per-compose env to `${{project.VAR}}` refs
 *                                 so Shared Env flows into compose interpolation
 *   Phase 3  syncDomains()        Shell out to scripts/sync-domains.mjs for each app
 *   Phase 4  deployAll()          compose.deploy in order, poll composeStatus
 *   Phase 5  verify()             HTTPS reachability + OIDC + Lago + RustFS checks
 *
 * Bootstrap env (set in this compose's per-compose env field in Dokploy UI):
 *   APP_DOMAIN, DOKPLOY_URL, DOKPLOY_API_KEY, DOKPLOY_ENVIRONMENT_ID
 *
 * Optional knobs (env vars):
 *   DEPLOY_DRY_RUN=true        Print plan, mutate nothing
 *   DEPLOY_SKIP_VERIFY=true    Skip Phase 5
 *   DEPLOY_TIMEOUT_SECS=600    Per-app deploy timeout (default 10 min)
 *   DEPLOY_ONLY=env|composes|domains|deploys|verify   Run a single phase
 *
 * Zero npm dependencies — only Node 22+ stdlib.
 */

import { spawnSync } from "node:child_process";
import { randomBytes, generateKeyPairSync } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { request as httpRequest } from "node:http";
import { resolve } from "node:path";

// ── Declarative manifest ────────────────────────────────────────────────────
// Defines the apps this orchestrator manages, in deploy order. composePath is
// relative to the repo root, matching Dokploy's `composePath` field.
const APPS = [
  { name: "infra",         composePath: "./dokploy/infra/docker-compose.yml",         hasDomains: true  },
  { name: "gateway",       composePath: "./dokploy/gateway/docker-compose.yml",       hasDomains: true  },
  { name: "web",           composePath: "./dokploy/web/docker-compose.yml",           hasDomains: true  },
  { name: "observability", composePath: "./dokploy/observability/docker-compose.yml", hasDomains: true  },
  { name: "capabilities",  composePath: "./dokploy/capabilities/docker-compose.yml",  hasDomains: false },
];

// External named Docker volumes the stacks expect to already exist. Declared
// `external: true` in the compose files so their lifecycle is decoupled from
// any deploy / project delete — they only go away on explicit `docker volume
// rm`. Phase 0 creates any that are missing; existing ones are left untouched.
const EXTERNAL_VOLUMES = [
  "conusai_postgres_data",
  "conusai_redis_data",
  "conusai_qdrant_data",
  "conusai_rustfs_data",
  "conusai_redb_data",
];

const DOCKER_SOCK = "/var/run/docker.sock";

// Secrets to auto-generate when absent from Shared Env. Format strings chosen
// to match the existing `generate-prod-env.mjs` output so we don't break any
// downstream regex/length validation in zitadel/lago/rustfs.
const SECRETS = {
  POSTGRES_PASSWORD:       () => randB64Url(30),  // ~40 chars
  ZITADEL_MASTERKEY:       () => randB64Url(24),  // exactly 32 chars
  LAGO_SECRET_KEY_BASE:    () => randHex(64),     // 128 hex chars
  LAGO_ENCRYPTION_DET_KEY: () => randB64Url(24),
  LAGO_ENCRYPTION_SALT:    () => randB64Url(24),
  LAGO_ENCRYPTION_KEY:     () => randB64Url(24),
  LAGO_RSA_PRIVATE_KEY:    () => base64(generateRsaPem(2048)),
  AWS_ACCESS_KEY_ID:       () => "rfs_" + randUpperAlnum(15),
  AWS_SECRET_ACCESS_KEY:   () => randB64Url(30),
  RUSTFS_IAM_ENC_KEY:      () => randB64Url(24),
  RUSTFS_WEBHOOK_SECRET:   () => randB64Url(32),
  UI_SESSION_KEY:          () => randHex(32),
  PLATFORM_ADMIN_TOKEN:    () => "pat_" + randB64Url(32),
};

// Variables derived from APP_DOMAIN. Recomputed every run so changing
// APP_DOMAIN in the orchestrator's per-compose env propagates everywhere
// (instead of being frozen to whatever .env.example shipped).
const DERIVED = (appDomain) => ({
  ZITADEL_ISSUER: `https://auth.${appDomain}`,
  COOKIE_DOMAIN:  `.${appDomain}`,
});

// ── CLI / env knobs ─────────────────────────────────────────────────────────
const DRY_RUN     = process.env.DEPLOY_DRY_RUN === "true";
const SKIP_VERIFY = process.env.DEPLOY_SKIP_VERIFY === "true";
const TIMEOUT_MS  = (Number(process.env.DEPLOY_TIMEOUT_SECS) || 600) * 1000;
const ONLY        = process.env.DEPLOY_ONLY || "";

// Resolve bind-mounted files (see docker-compose.yml). Fall back to local
// repo layout when running from an operator's laptop.
const ENV_EXAMPLE = pickExisting([
  "/app/.env.example",
  resolve(import.meta.dirname, "../../.env.example"),
]);
const SYNC_DOMAINS_SCRIPT = pickExisting([
  "/app/sync-domains.mjs",
  resolve(import.meta.dirname, "../../scripts/sync-domains.mjs"),
]);
const DOMAINS_YAML = pickExisting([
  "/app/domains.yaml",
  resolve(import.meta.dirname, "../../domains.yaml"),
]);

// Root holding sibling compose files (infra/gateway/web/...). Phase 2 reads
// each compose YAML to extract `${VAR}` references and wires Shared Env
// values into the per-compose env tab via `${{project.VAR}}` syntax.
const COMPOSES_ROOT = existsSync("/app/composes")
  ? "/app/composes"
  : resolve(import.meta.dirname, "../..");

// ── Phase 0: Ensure external Docker volumes exist ───────────────────────────
async function ensureVolumes() {
  if (!existsSync(DOCKER_SOCK)) {
    log(`⚠  ${DOCKER_SOCK} not mounted — skipping volume bootstrap.`);
    log(`   Mount it in this compose to enable auto-creation, or run`);
    log(`   docker volume create conusai_{postgres,redis,qdrant,rustfs,redb}_data on the host once.`);
    return;
  }
  for (const name of EXTERNAL_VOLUMES) {
    const existing = await dockerApi("GET", `/volumes/${encodeURIComponent(name)}`);
    if (existing.status === 200) {
      log(`  · ${name.padEnd(28)} exists`);
      continue;
    }
    if (existing.status !== 404) {
      fail(`docker GET /volumes/${name} → HTTP ${existing.status}: ${existing.body.slice(0, 200)}`);
    }
    if (DRY_RUN) {
      log(`  + ${name.padEnd(28)} CREATE (dry-run)`);
      continue;
    }
    const created = await dockerApi("POST", "/volumes/create", { Name: name });
    if (created.status !== 201) {
      fail(`docker POST /volumes/create ${name} → HTTP ${created.status}: ${created.body.slice(0, 200)}`);
    }
    log(`  + ${name.padEnd(28)} CREATED`);
  }
}

function dockerApi(method, path, body) {
  return new Promise((res, rej) => {
    const req = httpRequest(
      {
        socketPath: DOCKER_SOCK,
        method,
        path,
        headers: body
          ? { "content-type": "application/json", accept: "application/json" }
          : { accept: "application/json" },
      },
      (r) => {
        const chunks = [];
        r.on("data", (c) => chunks.push(c));
        r.on("end", () => res({ status: r.statusCode, body: Buffer.concat(chunks).toString("utf8") }));
      },
    );
    req.on("error", rej);
    if (body) req.write(JSON.stringify(body));
    req.end();
  });
}

// ── Phase 1: Shared Env ─────────────────────────────────────────────────────
async function ensureSharedEnv({ api, environmentId, appDomain }) {
  const example = parseDotenv(readFileSync(ENV_EXAMPLE, "utf8"));
  const envRecord = await api.query("environment.one", { environmentId });
  const current = parseDotenv(envRecord.env || "");

  // Bootstrap vars from this orchestrator's own env — these MUST end up in
  // Shared Env so domain-sync init containers in every other stack can call
  // back into the Dokploy API.
  const bootstrap = {
    APP_DOMAIN: process.env.APP_DOMAIN,
    DOKPLOY_URL: process.env.DOKPLOY_URL,
    DOKPLOY_API_KEY: process.env.DOKPLOY_API_KEY,
    DOKPLOY_ENVIRONMENT_ID: process.env.DOKPLOY_ENVIRONMENT_ID,
    ...DERIVED(appDomain),
  };

  const merged = { ...current };
  const actions = []; // [{ key, action, source }]
  const exampleKeys = new Set(Object.keys(example));
  for (const k of Object.keys(bootstrap)) exampleKeys.add(k);

  // Every `${VAR}` referenced by any compose must also exist in Project
  // Environment — Dokploy validates `${{project.VAR}}` references at deploy
  // time and aborts with "Invalid project environment variable: project.X"
  // if any is missing, even when the value is optional. We add blank entries
  // here so operators can fill real values later in the UI.
  const referenced = new Set();
  for (const app of APPS) {
    const composeFile = resolve(COMPOSES_ROOT, app.name, "docker-compose.yml");
    if (!existsSync(composeFile)) continue;
    for (const v of extractComposeVars(composeFile)) referenced.add(v);
  }
  for (const k of referenced) exampleKeys.add(k);

  for (const k of exampleKeys) {
    const existing = (merged[k] ?? "").trim();
    if (existing) continue; // never overwrite an existing value

    if (k in bootstrap && bootstrap[k]) {
      merged[k] = bootstrap[k];
      actions.push({ key: k, action: "set", source: "bootstrap" });
    } else if (SECRETS[k]) {
      merged[k] = SECRETS[k]();
      actions.push({ key: k, action: "gen", source: "auto-secret" });
    } else if ((example[k] ?? "").trim()) {
      merged[k] = example[k];
      actions.push({ key: k, action: "set", source: ".env.example" });
    } else if (!(k in merged)) {
      // Referenced by a compose but no value anywhere — write empty entry so
      // Dokploy's `${{project.X}}` validation passes. Operator fills in UI.
      merged[k] = "";
      actions.push({ key: k, action: "set", source: "placeholder (empty)" });
    }
  }

  if (actions.length === 0) {
    log("✓ Shared Env already complete — no changes");
    return;
  }

  for (const a of actions) {
    const masked = isSecret(a.key) ? "********" : merged[a.key];
    log(`  ${a.action === "gen" ? "+" : "·"} ${a.key.padEnd(28)} ${a.action} (${a.source}) ${a.action === "gen" ? "" : "= " + masked}`);
  }

  if (DRY_RUN) {
    log(`(dry-run) would PUT ${Object.keys(merged).length} keys to environment.env`);
    return;
  }

  const envText = renderDotenv(merged, envRecord.env || "");
  await api.mutate("environment.update", { environmentId, env: envText });
  log(`✓ PUT Shared Env (${actions.length} new keys, ${Object.keys(merged).length} total)`);
}

// ── Phase 2: Ensure compose services exist ──────────────────────────────────
async function ensureComposes({ api, environmentId, selfComposeId }) {
  if (!selfComposeId) {
    fail("Phase 2 needs a compose to inherit git source from. Create 'epifly-deploy' in the Dokploy UI first.");
  }
  const self = await api.query("compose.one", { composeId: selfComposeId });
  const inherit = {
    sourceType: self.sourceType,
    githubId: self.githubId,
    gitlabId: self.gitlabId,
    bitbucketId: self.bitbucketId,
    giteaId: self.giteaId,
    owner: self.owner,
    repository: self.repository,
    branch: self.branch,
    triggerType: self.triggerType ?? "tag",
    autoDeploy: true,
    composeType: "docker-compose",
  };
  log(`Source: ${self.sourceType} ${self.owner}/${self.repository}@${self.branch} (trigger=${inherit.triggerType})`);

  const search = await api.query("compose.search", { environmentId, limit: 100, offset: 0 });
  const existing = new Map((search.items ?? []).map((c) => [c.name, c]));

  for (const app of APPS) {
    let composeId;
    if (existing.has(app.name)) {
      composeId = existing.get(app.name).composeId;
      log(`  ✓ ${app.name.padEnd(15)} exists (composeId=${composeId})`);
    } else {
      log(`  + ${app.name.padEnd(15)} CREATE  ${app.composePath}`);
      if (DRY_RUN) continue;

      const created = await api.mutate("compose.create", {
        name: app.name,
        environmentId,
        description: `Managed by epifly-deploy. Source: ${app.composePath}`,
        composeType: "docker-compose",
      });
      composeId = created?.composeId ?? created?.json?.composeId;
      if (!composeId) fail(`compose.create returned no composeId: ${JSON.stringify(created)}`);

      // Attach git source via compose.update (compose.create only takes name+env).
      await api.mutate("compose.update", {
        composeId,
        ...inherit,
        composePath: app.composePath,
      });
      log(`    → composeId=${composeId}, git source attached`);
    }

    // Wire per-compose env: pure `${{project.VAR}}` references for every
    // ${VAR} the compose interpolates. Idempotent — full replace each run so
    // the manifest stays declarative.
    const composeFile = resolve(COMPOSES_ROOT, app.name, "docker-compose.yml");
    if (!existsSync(composeFile)) {
      log(`    ⚠ env wiring skipped — ${composeFile} not bind-mounted`);
      continue;
    }
    const vars = extractComposeVars(composeFile);
    const env = renderProjectRefs(vars);
    const preview = vars.slice(0, 4).join(", ") + (vars.length > 4 ? `, …+${vars.length - 4}` : "");
    log(`    env: ${vars.length} project refs (${preview})`);
    if (!DRY_RUN) {
      await api.mutate("compose.update", { composeId, env });
    }
  }
}

// Extract every `${VAR}`, `${VAR:-default}`, `${VAR:?msg}`, `${VAR-default}`,
// `${VAR?msg}` reference from a compose YAML. Returns a sorted unique list.
function extractComposeVars(composeFile) {
  const content = readFileSync(composeFile, "utf8");
  const re = /\$\{([A-Z_][A-Z0-9_]*)(?:[:?\-][^}]*)?\}/g;
  const vars = new Set();
  let m;
  while ((m = re.exec(content)) !== null) vars.add(m[1]);
  return [...vars].sort();
}

// Render a per-compose env block of pure project-level references. Dokploy
// expands `${{project.VAR}}` against Shared Env when writing the per-service
// .env file at deploy time, which compose then interpolates as `${VAR}`.
function renderProjectRefs(vars) {
  return vars.map((v) => `${v}=\${{project.${v}}}`).join("\n") + "\n";
}

// ── Phase 3: Sync domains ───────────────────────────────────────────────────
async function syncDomains({ dokployUrl, apiKey, environmentId }) {
  for (const app of APPS) {
    if (!app.hasDomains) {
      log(`  · ${app.name.padEnd(15)} no public domains in domains.yaml — skip`);
      continue;
    }
    log(`  ↻ ${app.name.padEnd(15)} sync-domains.mjs --app ${app.name}`);
    if (DRY_RUN) continue;

    const r = spawnSync(process.execPath, [SYNC_DOMAINS_SCRIPT, "--app", app.name], {
      stdio: "inherit",
      env: {
        ...process.env,
        DOKPLOY_URL: dokployUrl,
        DOKPLOY_API_KEY: apiKey,
        DOKPLOY_ENVIRONMENT_ID: environmentId,
      },
    });
    if (r.status !== 0) fail(`sync-domains failed for ${app.name} (exit ${r.status})`);
  }
}

// ── Phase 4: Deploy all in order ────────────────────────────────────────────
async function deployAll({ api, environmentId, selfComposeId }) {
  const search = await api.query("compose.search", { environmentId, limit: 100, offset: 0 });
  const byName = new Map((search.items ?? []).map((c) => [c.name, c]));

  for (const app of APPS) {
    const c = byName.get(app.name);
    if (!c) { log(`  ⚠ ${app.name} not found in project — skip`); continue; }
    if (c.composeId === selfComposeId) continue; // never redeploy self

    log(`  ▶ ${app.name.padEnd(15)} compose.deploy (composeId=${c.composeId})`);
    if (DRY_RUN) continue;

    await api.mutate("compose.deploy", { composeId: c.composeId });

    const final = await waitForStatus(api, c.composeId, app.name);
    if (final === "error" || final === "failed") fail(`${app.name} deploy ${final}`);
    log(`    → ${final}`);
  }
}

async function waitForStatus(api, composeId, name, timeoutMs = TIMEOUT_MS) {
  const start = Date.now();
  let last = "";
  while (Date.now() - start < timeoutMs) {
    const c = await api.query("compose.one", { composeId });
    const s = c.composeStatus;
    if (s !== last) { log(`    ${name}: ${s}`); last = s; }
    if (s === "done" || s === "error" || s === "failed") return s;
    await sleep(3000);
  }
  return "timeout";
}

// ── Phase 5: Verify ─────────────────────────────────────────────────────────
async function verifyAll({ appDomain }) {
  const checks = buildVerifyChecks(appDomain);
  const failures = [];
  for (const check of checks) {
    try {
      const ok = await runCheck(check);
      log(`  ${ok ? "✓" : "✗"} ${check.label.padEnd(50)} ${check.url}`);
      if (!ok) failures.push(check);
    } catch (e) {
      log(`  ✗ ${check.label.padEnd(50)} ${check.url}  (${e.message})`);
      failures.push(check);
    }
  }
  if (failures.length > 0) {
    fail(`${failures.length}/${checks.length} verification checks failed`);
  }
  log(`✓ all ${checks.length} verification checks passed`);
}

function buildVerifyChecks(appDomain) {
  return [
    { label: "web (root)",            url: `https://${appDomain}`,                                                    expectStatus: [200, 301, 302, 307, 308] },
    { label: "zitadel OIDC discovery", url: `https://auth.${appDomain}/.well-known/openid-configuration`,             expectStatus: [200], expectJsonKey: "issuer" },
    { label: "lago app",              url: `https://billing.${appDomain}`,                                            expectStatus: [200, 301, 302, 307, 308] },
    { label: "rustfs S3 (anon)",      url: `https://s3.${appDomain}/`,                                                expectStatus: [200, 403] },
    { label: "rustfs console",        url: `https://s3-console.${appDomain}`,                                         expectStatus: [200, 301, 302, 307, 308] },
    { label: "jaeger UI",             url: `https://traces.${appDomain}`,                                             expectStatus: [200, 301, 302, 307, 308] },
    { label: "gateway api healthz",   url: `https://api.${appDomain}/healthz`,                                        expectStatus: [200, 404] }, // 404 acceptable until route is added
  ];
}

async function runCheck({ url, expectStatus, expectJsonKey }) {
  const ac = new AbortController();
  const t = setTimeout(() => ac.abort(), 15000);
  try {
    const res = await fetch(url, { redirect: "manual", signal: ac.signal });
    if (!expectStatus.includes(res.status)) return false;
    if (expectJsonKey) {
      const body = await res.json().catch(() => null);
      if (!body || !(expectJsonKey in body)) return false;
    }
    return true;
  } finally {
    clearTimeout(t);
  }
}

// ── Helpers ─────────────────────────────────────────────────────────────────
function loadConfig() {
  const dokployUrl = (process.env.DOKPLOY_URL || "").replace(/\/+$/, "");
  const apiKey = process.env.DOKPLOY_API_KEY;
  const environmentId = process.env.DOKPLOY_ENVIRONMENT_ID;
  const appDomain = process.env.APP_DOMAIN;
  if (!dokployUrl) fail("DOKPLOY_URL is required");
  if (!apiKey)     fail("DOKPLOY_API_KEY is required");
  if (!environmentId) fail("DOKPLOY_ENVIRONMENT_ID is required");
  if (!appDomain)  fail("APP_DOMAIN is required");
  return { dokployUrl, apiKey, environmentId, appDomain };
}

function banner(cfg) {
  log("─".repeat(72));
  log("  epifly-deploy — Dokploy project orchestrator");
  log("─".repeat(72));
  log(`  Dokploy:        ${cfg.dokployUrl}`);
  log(`  Environment:    ${cfg.environmentId}`);
  log(`  APP_DOMAIN:     ${cfg.appDomain}`);
  log(`  Mode:           ${DRY_RUN ? "DRY RUN" : "APPLY"}`);
  log(`  Phase filter:   ${ONLY || "(all phases)"}`);
  log("─".repeat(72));
}

function section(title) {
  log("");
  log(`▼ ${title}`);
}

function makeClient(baseUrl, apiKey) {
  return {
    async query(procedure, input) {
      const url = `${baseUrl}/api/trpc/${procedure}?input=${encodeURIComponent(JSON.stringify({ json: input }))}`;
      const res = await fetch(url, { headers: { "x-api-key": apiKey, accept: "application/json" } });
      return unwrap(res, procedure);
    },
    async mutate(procedure, input) {
      const res = await fetch(`${baseUrl}/api/trpc/${procedure}`, {
        method: "POST",
        headers: { "x-api-key": apiKey, "content-type": "application/json", accept: "application/json" },
        body: JSON.stringify({ json: input }),
      });
      return unwrap(res, procedure);
    },
  };
}

async function unwrap(res, procedure) {
  const text = await res.text();
  let body;
  try { body = text ? JSON.parse(text) : null; }
  catch { fail(`${procedure}: non-JSON response (${res.status}): ${text.slice(0, 200)}`); }
  if (!res.ok) {
    const msg = body?.error?.json?.message ?? body?.message ?? text;
    fail(`${procedure} → HTTP ${res.status}: ${msg}`);
  }
  return body?.result?.data?.json ?? body?.result?.data ?? body;
}

function parseDotenv(text) {
  const out = {};
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq < 0) continue;
    const k = line.slice(0, eq).trim();
    let v = line.slice(eq + 1).trim();
    if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) {
      v = v.slice(1, -1);
    }
    out[k] = v;
  }
  return out;
}

// Render env preserving order from prior text where possible; new keys appended.
function renderDotenv(merged, priorText) {
  const seen = new Set();
  const out = [];
  for (const raw of priorText.split(/\r?\n/)) {
    const m = raw.match(/^\s*([A-Z][A-Z0-9_]*)\s*=/);
    if (m) {
      const k = m[1];
      if (k in merged) { out.push(`${k}=${merged[k]}`); seen.add(k); }
      else out.push(raw); // unknown key — preserve
    } else {
      out.push(raw); // comment / blank
    }
  }
  for (const k of Object.keys(merged)) {
    if (!seen.has(k)) out.push(`${k}=${merged[k]}`);
  }
  return out.join("\n");
}

function pickExisting(paths) {
  for (const p of paths) if (existsSync(p)) return p;
  fail(`None of these paths exist: ${paths.join(", ")}`);
}

function isSecret(k) {
  return /PASSWORD|SECRET|TOKEN/.test(k) || /_KEY(_|$)/.test(k);
}

function randB64Url(bytes) {
  return randomBytes(bytes).toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function randHex(bytes) { return randomBytes(bytes).toString("hex"); }
function randUpperAlnum(n) {
  const charset = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  const buf = randomBytes(n);
  let s = "";
  for (let i = 0; i < n; i++) s += charset[buf[i] % charset.length];
  return s;
}
function base64(s) { return Buffer.from(s, "utf8").toString("base64"); }
function generateRsaPem(modulusLength) {
  return generateKeyPairSync("rsa", { modulusLength }).privateKey.export({ type: "pkcs8", format: "pem" });
}

function sleep(ms) { return new Promise((r) => setTimeout(r, ms)); }
function log(...args) { console.log(...args); }
function fail(msg) { console.error(`✗ ${msg}`); process.exit(1); }

// ── Self-discovery: find our own composeId from the env API ────────────────
async function discoverSelfComposeId(api, environmentId) {
  const r = await api.query("compose.search", { environmentId, limit: 100, offset: 0 });
  const self = (r.items ?? []).find((c) => c.name === "epifly-deploy");
  return self?.composeId ?? null;
}

// Wrap main to inject selfComposeId after loadConfig (needs API call).
(async () => {
  try {
    // Pre-flight: load config, build client, look up self composeId.
    const cfg = loadConfig();
    const api = makeClient(cfg.dokployUrl, cfg.apiKey);
    cfg.selfComposeId = await discoverSelfComposeId(api, cfg.environmentId);
    if (!cfg.selfComposeId) {
      fail(
        "No compose named 'epifly-deploy' found in this environment.\n" +
        "    Create it in the Dokploy UI first (Project → Add Compose → Source = this repo,\n" +
        "    Compose Path = ./dokploy/epifly-deploy/docker-compose.yml) and deploy it once.",
      );
    }
    // Re-bind everything and run phases (replaces main()'s loadConfig).
    banner(cfg);
    const ctx = { ...cfg, api };
    const phases = {
      volumes:  () => ensureVolumes(),
      env:      () => ensureSharedEnv(ctx),
      composes: () => ensureComposes(ctx),
      domains:  () => syncDomains(ctx),
      deploys:  () => deployAll(ctx),
      verify:   () => SKIP_VERIFY ? log("skipped (DEPLOY_SKIP_VERIFY=true)") : verifyAll(ctx),
    };
    const order = ["volumes", "env", "composes", "domains", "deploys", "verify"];
    const run = ONLY ? [ONLY] : order;
    for (const name of run) {
      if (!phases[name]) fail(`Unknown phase "${name}". Valid: ${order.join(", ")}`);
      section(`Phase: ${name}`);
      await phases[name]();
    }
    section("Done");
    log(`✓ epifly-deploy completed${DRY_RUN ? " (DRY RUN)" : ""}`);
  } catch (err) {
    console.error(err);
    process.exit(1);
  }
})();
