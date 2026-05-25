#!/usr/bin/env node
/**
 * epifly-deploy — Dokploy project orchestrator.
 *
 * Runs once per deploy of the `epifly-deploy` compose service. Reconciles the
 * entire Epifly project against this repo's declarative configuration:
 *
 *   Phase 0  ensureVolumes()      Pre-create `external: true` named Docker
 *                                 volumes via the Engine API (data-safe)
 *   Phase 1  ensureSharedEnv()    Generate any missing secrets, PUT project.env
 *                                 (NOT environment.env — `${{project.X}}` refs
 *                                 resolve against the project-level env, see
 *                                 Dokploy/dokploy packages/server/src/utils/
 *                                 docker/utils.ts → prepareEnvironmentVariables)
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
 * Shared logic lives in dokploy/lib/*.mjs — imported dynamically so this
 * file works both inside the container (/app/lib) and from a local checkout.
 */

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// ── Resolve shared lib directory ────────────────────────────────────────────
// Container: /app/scripts/deploy.mjs → /app/lib  (../lib relative to script)
// Local dev:  dokploy/epifly-deploy/scripts/     → dokploy/lib  (../../lib)
const __dir = fileURLToPath(new URL(".", import.meta.url));
function findLib() {
  for (const p of [resolve(__dir, "../lib"), resolve(__dir, "../../lib")]) {
    if (existsSync(p)) return p;
  }
  console.error("✗ Cannot find dokploy/lib. Add bind-mount ../lib:/app/lib:ro to docker-compose.yml.");
  process.exit(1);
}
const LIB = findLib();
const libUrl = (mod) => pathToFileURL(resolve(LIB, mod)).href;

// ── Dynamic imports from the shared library ─────────────────────────────────
const { APPS, EXTERNAL_VOLUMES, DOCKER_SOCK, DERIVED } = await import(libUrl("manifest.mjs"));
const { SECRETS, STATEFUL_SECRETS } = await import(libUrl("secrets.mjs"));
const { makeClient } = await import(libUrl("dokploy-client.mjs"));
const { parseDotenv, renderDotenv, isSecret } = await import(libUrl("dotenv.mjs"));
const { extractComposeVars, renderProjectRefs } = await import(libUrl("compose-vars.mjs"));
const { buildVerifyChecks, runCheck } = await import(libUrl("verify.mjs"));
const { dockerApi, listExistingVolumes } = await import(libUrl("docker.mjs"));

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

// ── Phase 1: Shared Env ─────────────────────────────────────────────────────
// Writes to the **project**-level env (projects.env), because Dokploy resolves
// `${{project.X}}` refs in per-compose env tabs against `compose.environment.
// project.env`, NOT against `compose.environment.env`. Writing to the wrong
// field causes deploys to abort with
//   "Invalid project environment variable: project.X"
// even though the key exists in the (wrong) env-level store.
async function ensureSharedEnv({ api, projectId, appDomain, preExistingVolumes }) {
  const example = parseDotenv(readFileSync(ENV_EXAMPLE, "utf8"));
  const projectRecord = await api.query("project.one", { projectId });
  const current = parseDotenv(projectRecord.env || "");

  // Bootstrap vars from this orchestrator's own env — these MUST end up in
  // Shared Env so domain-sync init containers in every other stack can call
  // back into the Dokploy API.
  //
  // Operator-meaningful credentials (POSTGRES_USER/PASSWORD/DB, RustFS S3
  // admin keys, PLATFORM_ADMIN_TOKEN, …) can ALSO be pinned here. When set,
  // they survive a project recreate / Shared-Env wipe and re-bootstrap to
  // the same values on the next deploy — so the orchestrator's compose env
  // becomes the canonical source of truth and the credentials baked into
  // stateful volumes (postgres, rustfs) never drift.
  //
  // Undefined / empty values are skipped by the merge loop below
  // (`if (k in bootstrap && bootstrap[k])`), so leaving them unset falls
  // through to the SECRETS auto-generator (itself guarded by
  // STATEFUL_SECRETS against silent regeneration when data exists).
  //
  // Opaque crypto material (encryption keys, RSA PEM, master keys) is NOT
  // pinned here — no human types those; they're auto-generated once and
  // protected by the volume-existence guard from then on.
  const bootstrap = {
    APP_DOMAIN: process.env.APP_DOMAIN,
    DOKPLOY_URL: process.env.DOKPLOY_URL,
    DOKPLOY_API_KEY: process.env.DOKPLOY_API_KEY,
    DOKPLOY_ENVIRONMENT_ID: process.env.DOKPLOY_ENVIRONMENT_ID,
    // Postgres (shared by Zitadel + Lago + future apps)
    POSTGRES_USER: process.env.POSTGRES_USER,
    POSTGRES_PASSWORD: process.env.POSTGRES_PASSWORD,
    POSTGRES_DB: process.env.POSTGRES_DB,
    // RustFS S3 admin (bound to conusai_rustfs_data)
    AWS_ACCESS_KEY_ID: process.env.AWS_ACCESS_KEY_ID,
    AWS_SECRET_ACCESS_KEY: process.env.AWS_SECRET_ACCESS_KEY,
    // Platform admin bearer token (used by CLIs / dashboards)
    PLATFORM_ADMIN_TOKEN: process.env.PLATFORM_ADMIN_TOKEN,
    // External API keys — set in per-compose env to survive project recreate
    ANTHROPIC_API_KEY: process.env.ANTHROPIC_API_KEY,
    ...DERIVED(appDomain),
  };

  // Stateful volumes that existed on the host BEFORE this orchestrator
  // run started (snapshot taken in main before Phase 0). Used below to
  // refuse silent regeneration of secrets bound to live data. We do NOT
  // re-query here, because Phase 0 (`ensureVolumes`) may have just
  // created these volumes empty in this same run — querying now would
  // always find them and falsely trip the guard on fresh deploys.
  // Fallback to a fresh query when the snapshot wasn't passed (e.g.
  // `DEPLOY_ONLY=env` invoked outside the main() phase loop).
  const existingVolumes = preExistingVolumes
    ?? await listExistingVolumes(new Set(Object.values(STATEFUL_SECRETS)));

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
      const boundVolume = STATEFUL_SECRETS[k];
      if (boundVolume && existingVolumes.has(boundVolume)) {
        fail(
          `Refusing to regenerate stateful secret ${k}: volume ${boundVolume} ` +
          `already exists on the host, so data on it was encrypted/hashed with the ` +
          `previous value. Regenerating would corrupt that data.\n\n` +
          `Either:\n` +
          `  1. Restore the original value in Dokploy Shared Env (project.env), or\n` +
          `  2. Wipe the volume explicitly on the host:\n` +
          `       docker volume rm ${boundVolume}\n` +
          `     and re-run the deploy (you will lose all data in that volume).`,
        );
      }
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
    log(`(dry-run) would PUT ${Object.keys(merged).length} keys to project.env`);
    return;
  }

  const envText = renderDotenv(merged, projectRecord.env || "");
  await api.mutate("project.update", { projectId, env: envText });
  log(`✓ PUT Shared Env to project.env (${actions.length} new keys, ${Object.keys(merged).length} total)`);
}

// ── Phase 2: Ensure compose services exist ──────────────────────────────────
async function ensureComposes({ api, environmentId, selfComposeId }) {
  if (!selfComposeId) {
    fail("Phase 2 needs a compose to inherit git source from. Create 'epifly-deploy' in the Dokploy UI first.");
  }
  const self = await api.query("compose.one", { composeId: selfComposeId });
  const targetServerId = await resolveTargetServerId(api, self);
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
        ...(targetServerId ? { serverId: targetServerId } : {}),
      });
      composeId = created?.composeId ?? created?.json?.composeId;
      if (!composeId) fail(`compose.create returned no composeId: ${JSON.stringify(created)}`);

      // Attach git source via compose.update (second step after compose.create).
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

async function resolveTargetServerId(api, selfCompose) {
  if (selfCompose?.serverId) return selfCompose.serverId;

  const servers = await api.query("server.all", {});
  const list = Array.isArray(servers) ? servers : [];
  const active = list.find((s) => s?.serverStatus === "active");
  if (active?.serverId) {
    log(`  · epifly-deploy has no serverId; using active server ${active.name || active.serverId} for new composes`);
    return active.serverId;
  }

  const fallback = list[0]?.serverId;
  if (fallback) {
    log(`  · epifly-deploy has no serverId; using fallback server ${list[0]?.name || fallback} for new composes`);
    return fallback;
  }

  fail(
    "No Dokploy servers available for compose.create. Add and setup a server first, then rerun deploy.",
  );
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
  // projectId is resolved at runtime from environment.one (we only have the
  // environmentId in our bootstrap env).
  return { dokployUrl, apiKey, environmentId, projectId: null, appDomain };
}

function banner(cfg) {
  log("─".repeat(72));
  log("  epifly-deploy — Dokploy project orchestrator");
  log("─".repeat(72));
  log(`  Dokploy:        ${cfg.dokployUrl}`);
  log(`  Project:        ${cfg.projectId}`);
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


function pickExisting(paths) {
  for (const p of paths) if (existsSync(p)) return p;
  console.error(`✗ None of these paths exist: ${paths.join(", ")}`);
  process.exit(1);
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
    // Pre-flight: load config, build client, look up projectId + self composeId.
    const cfg = loadConfig();
    const api = makeClient(cfg.dokployUrl, cfg.apiKey);
    // Resolve projectId from environmentId — we don't get it in bootstrap env
    // because Dokploy's UI only exposes the environment URL.
    const envRec = await api.query("environment.one", { environmentId: cfg.environmentId });
    cfg.projectId = envRec.projectId ?? envRec.project?.projectId;
    if (!cfg.projectId) fail(`environment.one for ${cfg.environmentId} returned no projectId`);
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
    // Snapshot which stateful volumes pre-existed BEFORE Phase 0 runs.
    // Phase 1's STATEFUL_SECRETS guard uses this snapshot so it only refuses
    // to regenerate when the bound volume genuinely carries data from a
    // previous deploy — not when Phase 0 just created the volume empty in
    // this same run.
    const preExistingVolumes = await listExistingVolumes(
      new Set(Object.values(STATEFUL_SECRETS)),
    );
    const ctx = { ...cfg, api, preExistingVolumes };
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
