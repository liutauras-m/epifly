#!/usr/bin/env node

// src/cli.ts
import { Command } from "commander";

// ../../dokploy/lib/dotenv.mjs
function parseDotenv(text3) {
  const out = {};
  for (const raw of text3.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq < 0) continue;
    const k = line.slice(0, eq).trim();
    let v = line.slice(eq + 1).trim();
    if (v.startsWith('"') && v.endsWith('"') || v.startsWith("'") && v.endsWith("'")) {
      v = v.slice(1, -1);
    }
    out[k] = v;
  }
  return out;
}
function renderDotenv(merged, priorText) {
  const seen = /* @__PURE__ */ new Set();
  const out = [];
  for (const raw of priorText.split(/\r?\n/)) {
    const m = raw.match(/^\s*([A-Z][A-Z0-9_]*)\s*=/);
    if (m) {
      const k = m[1];
      if (k in merged) {
        out.push(`${k}=${merged[k]}`);
        seen.add(k);
      } else {
        out.push(raw);
      }
    } else {
      out.push(raw);
    }
  }
  for (const k of Object.keys(merged)) {
    if (!seen.has(k)) out.push(`${k}=${merged[k]}`);
  }
  return out.join("\n");
}
function isSecret(k) {
  return /PASSWORD|SECRET|TOKEN/.test(k) || /_KEY(_|$)/.test(k);
}

// ../../dokploy/lib/manifest.mjs
var APPS = [
  { name: "infra", composePath: "./dokploy/infra/docker-compose.yml", hasDomains: true },
  { name: "gateway", composePath: "./dokploy/gateway/docker-compose.yml", hasDomains: true },
  { name: "web", composePath: "./dokploy/web/docker-compose.yml", hasDomains: true },
  { name: "observability", composePath: "./dokploy/observability/docker-compose.yml", hasDomains: true },
  { name: "capabilities", composePath: "./dokploy/capabilities/docker-compose.yml", hasDomains: false }
];
var EXTERNAL_VOLUMES = [
  "conusai_postgres_data",
  "conusai_redis_data",
  "conusai_qdrant_data",
  "conusai_rustfs_data",
  "conusai_redb_data",
  "conusai_lago_storage_data"
];

// src/lib/config.ts
import { existsSync, readFileSync, writeFileSync } from "fs";
import { homedir } from "os";
import { resolve } from "path";
var CONFIG_SEARCH = [
  resolve(process.cwd(), ".dokploy"),
  resolve(process.cwd(), ".epifly.json"),
  resolve(homedir(), ".dokploy")
];
function readConfigFile(path) {
  if (!existsSync(path)) return {};
  const raw = readFileSync(path, "utf8");
  try {
    return { ...JSON.parse(raw), _source: path };
  } catch {
    const out = { _source: path };
    for (const line of raw.split(/\r?\n/)) {
      const t = line.trim();
      if (!t || t.startsWith("#")) continue;
      const m = t.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/);
      if (!m) continue;
      let value = m[2].trim();
      if (value.startsWith('"') && value.endsWith('"') || value.startsWith("'") && value.endsWith("'")) {
        value = value.slice(1, -1);
      }
      if (m[1] === "DOKPLOY_URL") out.dokployUrl = value;
      if (m[1] === "DOKPLOY_API_KEY") out.apiKey = value;
      if (m[1] === "DOKPLOY_ENVIRONMENT_ID") out.environmentId = value;
      if (m[1] === "APP_DOMAIN") out.appDomain = value;
    }
    if (out.dokployUrl || out.apiKey || out.environmentId || out.appDomain) {
      return out;
    }
    throw new Error(`Failed to parse config file ${path}: invalid JSON or env format`);
  }
}
function readPartialConfig(flags = {}) {
  const explicitConfigPath = flags.config ? resolve(process.cwd(), flags.config) : void 0;
  const configPath = explicitConfigPath || process.env.EPIFLY_CONFIG || CONFIG_SEARCH.find((p3) => existsSync(p3));
  const file = configPath ? readConfigFile(configPath) : {};
  const env = {
    ...process.env.DOKPLOY_URL ? { dokployUrl: process.env.DOKPLOY_URL } : {},
    ...process.env.DOKPLOY_API_KEY ? { apiKey: process.env.DOKPLOY_API_KEY } : {},
    ...process.env.DOKPLOY_ENVIRONMENT_ID ? { environmentId: process.env.DOKPLOY_ENVIRONMENT_ID } : {},
    ...process.env.APP_DOMAIN ? { appDomain: process.env.APP_DOMAIN } : {}
  };
  const merged = {
    repoRoot: process.cwd(),
    ...file,
    ...env,
    ...flags.dokployUrl ? { dokployUrl: flags.dokployUrl } : {},
    ...flags.apiKey ? { apiKey: flags.apiKey } : {},
    ...flags.environmentId ? { environmentId: flags.environmentId } : {},
    ...flags.appDomain ? { appDomain: flags.appDomain } : {},
    ...flags.repoRoot ? { repoRoot: flags.repoRoot } : {}
  };
  return merged;
}
function loadConfig(flags = {}) {
  const cfg = readPartialConfig(flags);
  const required = [
    "dokployUrl",
    "apiKey",
    "environmentId",
    "appDomain"
  ];
  const missing = required.filter((k) => !cfg[k]);
  if (missing.length > 0) {
    throw new Error(
      `Missing required config: ${missing.join(", ")}

Set them via:
  \u2022 epifly init            (interactive wizard)
  \u2022 .dokploy config file   (JSON)
  \u2022 Environment variables  (DOKPLOY_URL, DOKPLOY_API_KEY, DOKPLOY_ENVIRONMENT_ID, APP_DOMAIN)`
    );
  }
  return {
    dokployUrl: cfg.dokployUrl.replace(/\/+$/, ""),
    apiKey: cfg.apiKey,
    environmentId: cfg.environmentId,
    appDomain: cfg.appDomain,
    repoRoot: cfg.repoRoot ?? process.cwd()
  };
}

// src/lib/dokploy.ts
async function get(cfg, procedure, input) {
  const base = cfg.dokployUrl.replace(/\/+$/, "");
  const params = new URLSearchParams();
  for (const [k, v] of Object.entries(input ?? {})) {
    if (v !== void 0 && v !== null) params.set(k, String(v));
  }
  const qs = params.toString();
  const url = `${base}/api/${procedure}${qs ? `?${qs}` : ""}`;
  const res = await fetch(url, {
    headers: { "x-api-key": cfg.apiKey, accept: "application/json" }
  });
  return unwrap(res, procedure);
}
async function post(cfg, procedure, input) {
  const base = cfg.dokployUrl.replace(/\/+$/, "");
  const res = await fetch(`${base}/api/${procedure}`, {
    method: "POST",
    headers: {
      "x-api-key": cfg.apiKey,
      "content-type": "application/json",
      accept: "application/json"
    },
    body: JSON.stringify(input ?? {})
  });
  return unwrap(res, procedure);
}
async function unwrap(res, procedure) {
  const text3 = await res.text();
  let body = null;
  try {
    body = text3 ? JSON.parse(text3) : null;
  } catch {
    throw new Error(`${procedure}: non-JSON response (HTTP ${res.status}): ${text3.slice(0, 200)}`);
  }
  if (!res.ok) {
    const msg = body?.message ?? body?.error ?? text3.slice(0, 200);
    throw new Error(`${procedure} \u2192 HTTP ${res.status}: ${msg}`);
  }
  return body;
}
async function searchComposes(cfg, params) {
  return get(cfg, "compose.search", params);
}
async function getCompose(cfg, composeId) {
  return get(cfg, "compose.one", { composeId });
}
async function updateCompose(cfg, params) {
  return post(cfg, "compose.update", params);
}
async function triggerDeploy(cfg, composeId) {
  return post(cfg, "compose.deploy", { composeId });
}
async function deleteCompose(cfg, params) {
  return post(cfg, "compose.delete", params);
}
async function readComposeLogs(cfg, params) {
  return get(cfg, "compose.readLogs", params);
}
async function listDeploymentsByCompose(cfg, composeId) {
  return await get(cfg, "deployment.allByCompose", { composeId }) ?? [];
}
async function listDeploymentsByServer(cfg, serverId) {
  return await get(cfg, "deployment.allByServer", { serverId }) ?? [];
}
async function listDeploymentQueue(cfg) {
  return await get(cfg, "deployment.queueList") ?? [];
}
async function listServers(cfg) {
  return await get(cfg, "server.all") ?? [];
}
async function getServerCount(cfg) {
  const result = await get(cfg, "server.count");
  if (typeof result === "number") return result;
  if (typeof result?.count === "number") return result.count;
  return Number(result ?? 0) || 0;
}
async function getServiceContainersByAppName(cfg, params) {
  return await get(cfg, "docker.getServiceContainersByAppName", params) ?? [];
}
async function getContainersByAppNameMatch(cfg, params) {
  return await get(cfg, "docker.getContainersByAppNameMatch", params) ?? [];
}
async function getAllProjects(cfg) {
  return await get(cfg, "project.all") ?? [];
}
async function getProject(cfg, projectId) {
  return get(cfg, "project.one", { projectId });
}
async function updateProject(cfg, params) {
  return post(cfg, "project.update", params);
}
async function getEnvironment(cfg, environmentId) {
  return get(cfg, "environment.one", { environmentId });
}
async function listEnvironments(cfg, params) {
  const result = await get(
    cfg,
    "environment.search",
    params
  );
  return result?.items ?? result ?? [];
}
async function getDomainsByCompose(cfg, composeId) {
  return await get(cfg, "domain.byComposeId", { composeId }) ?? [];
}
async function deleteDomain(cfg, domainId) {
  return post(cfg, "domain.delete", { domainId });
}

// src/lib/ui.ts
import pc from "picocolors";
function banner(title) {
  const line = "\u2500".repeat(64);
  console.log(pc.dim(line));
  console.log(`  ${pc.bold(pc.cyan("epifly"))} ${pc.dim("\u203A")} ${pc.bold(title)}`);
  console.log(pc.dim(line));
}
function ok(msg) {
  console.log(`${pc.green("\u2713")} ${msg}`);
}
function warn(msg) {
  console.warn(`${pc.yellow("\u26A0")} ${pc.yellow(msg)}`);
}
function err(msg) {
  console.error(`${pc.red("\u2717")} ${pc.red(msg)}`);
}
function info(msg) {
  console.log(`${pc.blue("\xB7")} ${msg}`);
}
function section(title) {
  console.log();
  console.log(pc.bold(pc.dim(`\u25BC ${title}`)));
}
function table(rows) {
  const col1 = Math.max(...rows.map(([a]) => a.length), 0);
  const col2 = Math.max(...rows.map(([, b]) => b.length), 0);
  for (const [a, b, c] of rows) {
    const status = c === "ok" ? pc.green("ok") : c === "error" ? pc.red("error") : c ? pc.dim(c) : "";
    console.log(`  ${a.padEnd(col1)}  ${b.padEnd(col2)}  ${status}`);
  }
}
function fatal(msg, hint) {
  err(msg);
  if (hint) console.error(pc.dim(`  ${hint}`));
  process.exit(1);
}

// src/lib/log-tail.ts
async function tailDeployLogs(cfg, composeId, timeoutMs = 6e5) {
  const start = Date.now();
  let lastStatus = "";
  let idleSince = null;
  let lastQueueState = "";
  while (Date.now() - start < timeoutMs) {
    const compose = await getCompose(cfg, composeId);
    const status = compose?.composeStatus ?? "unknown";
    if (status !== lastStatus) {
      info(`deploy status: ${status}`);
      lastStatus = status;
    }
    if (status === "done" || status === "error" || status === "failed") {
      return status;
    }
    if (status === "idle") {
      idleSince = idleSince ?? Date.now();
      const idleMs = Date.now() - idleSince;
      if (idleMs >= 15e3) {
        try {
          const queue = await listDeploymentQueue(cfg);
          const pending = queue.filter((j) => j?.data?.composeId === composeId);
          if (pending.length > 0) {
            const latest = pending[0];
            const queueState = String(latest?.state ?? "unknown");
            if (queueState !== lastQueueState) {
              info(`deployment queue: ${queueState} (${pending.length} job(s))`);
              lastQueueState = queueState;
            }
            if (queueState === "waiting" && idleMs >= 6e4) {
              warn("Deployment is queued but not being processed by Dokploy worker.");
              return "queue_stalled";
            }
          }
        } catch {
        }
      }
    } else {
      idleSince = null;
      lastQueueState = "";
    }
    await sleep(3e3);
  }
  warn("Timed out waiting for deploy to complete.");
  return "timeout";
}
function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

// src/commands/deploy.ts
var ALLOWED_PHASES = ["volumes", "env", "composes", "domains", "deploys", "verify"];
function registerDeploy(program2) {
  program2.command("deploy").description("Trigger the epifly-deploy orchestrator and stream logs").option("--config <path>", "Path to .dokploy config file").option("--dry-run", "Pass DEPLOY_DRY_RUN=true to the orchestrator").option("--phase <name>", "Run a single phase (volumes|env|composes|domains|deploys|verify)").option("--skip-verify", "Skip Phase 5 verify").option("--timeout <secs>", "Per-deploy timeout in seconds", "600").action(async (opts) => {
    let cfg;
    try {
      cfg = loadConfig({ config: opts.config });
    } catch (e) {
      fatal(e.message);
    }
    banner("deploy");
    try {
      const serverCount = await getServerCount(cfg);
      if (serverCount <= 0) {
        fatal(
          "Dokploy has no servers configured for this API key/project.",
          "Add/connect a server in Dokploy, then rerun `epifly deploy`."
        );
      }
    } catch (e) {
      warn(`Could not verify Dokploy server count: ${e.message}`);
    }
    let composeId;
    try {
      const search = await searchComposes(cfg, {
        environmentId: cfg.environmentId,
        limit: 100,
        offset: 0
      });
      const self = (search?.items ?? []).find((c) => c.name === "epifly-deploy");
      if (!self) {
        fatal(
          "No 'epifly-deploy' compose found in this environment.",
          "Run `epifly init` or create it manually in the Dokploy UI."
        );
      }
      composeId = self.composeId;
    } catch (e) {
      fatal(`Failed to locate epifly-deploy compose: ${e.message}`);
    }
    const envOverrides = {};
    if (opts.dryRun) envOverrides.DEPLOY_DRY_RUN = "true";
    if (opts.phase) {
      if (!ALLOWED_PHASES.includes(opts.phase)) {
        fatal(`Invalid --phase '${opts.phase}'.`, `Allowed phases: ${ALLOWED_PHASES.join("|")}`);
      }
      envOverrides.DEPLOY_ONLY = opts.phase;
    }
    if (opts.skipVerify) envOverrides.DEPLOY_SKIP_VERIFY = "true";
    let priorComposeEnv = "";
    let appliedTemporaryOverrides = false;
    if (Object.keys(envOverrides).length > 0) {
      info(`Setting overrides: ${JSON.stringify(envOverrides)}`);
      try {
        const compose = await getCompose(cfg, composeId);
        priorComposeEnv = compose?.env ?? "";
        const merged = {
          ...parseDotenv(priorComposeEnv),
          ...envOverrides
        };
        const nextEnv = renderDotenv(merged, priorComposeEnv);
        await updateCompose(cfg, { composeId, env: nextEnv });
        appliedTemporaryOverrides = true;
        info("Applied temporary compose env overrides for this deploy.");
      } catch (e) {
        fatal(`Failed to apply temporary deploy overrides: ${e.message}`);
      }
    }
    section("Triggering deploy");
    info(`Compose: epifly-deploy (${composeId})`);
    let finalStatus = "unknown";
    try {
      const trigger = await triggerDeploy(cfg, composeId);
      if (trigger?.success === false) {
        fatal(`Dokploy rejected deploy trigger: ${trigger?.message || "unknown error"}`);
      }
      ok("Deploy triggered \u2014 tailing logs\u2026");
      console.log();
      const timeoutMs = Number(opts.timeout) * 1e3;
      finalStatus = await tailDeployLogs(cfg, composeId, timeoutMs);
    } catch (e) {
      fatal(`Failed to trigger deploy: ${e.message}`);
    } finally {
      if (appliedTemporaryOverrides) {
        try {
          await updateCompose(cfg, { composeId, env: priorComposeEnv });
          info("Restored compose env after deploy.");
        } catch (e) {
          warn(`Failed to restore compose env overrides: ${e.message}`);
          warn("Please verify DEPLOY_* variables in the epifly-deploy compose env.");
        }
      }
    }
    console.log();
    if (finalStatus === "done") {
      await ensureManagedServicesRunning(cfg, cfg.environmentId, Number(opts.timeout) * 1e3);
      ok("Deploy completed successfully");
    } else if (finalStatus === "queue_stalled") {
      const latest = await getLatestDeploymentSummary(cfg, composeId);
      warn("Deploy queue appears stalled (job is waiting and compose stayed idle).");
      if (latest) {
        info(`Latest deployment: ${latest.status} (${latest.deploymentId})`);
        if (latest.createdAt) info(`Created at: ${latest.createdAt}`);
      }
      fatal(
        "Dokploy worker is not processing deployment queue.",
        "Check Dokploy worker/queue health, then retry `epifly deploy`."
      );
    } else if (finalStatus === "timeout") {
      warn("Timed out waiting for deploy. Check the Dokploy UI for status.");
      process.exit(1);
    } else {
      const latest = await getLatestDeploymentSummary(cfg, composeId);
      if (latest) {
        info(`Latest deployment: ${latest.status} (${latest.deploymentId})`);
        if (latest.errorMessage) warn(`Error: ${latest.errorMessage}`);
        if (latest.logPath) info(`Log path: ${latest.logPath}`);
        if (!await hasComposeServerBinding(cfg, composeId)) {
          warn(
            "Latest deployment has no serverId. This usually means the compose is not bound to a Dokploy server."
          );
          warn(
            "Recreate epifly-deploy compose with a valid server in Dokploy, then rerun epifly deploy."
          );
        }
      }
      await printServerDiagnosticSummary(cfg);
      fatal(`Deploy ${finalStatus}`);
    }
  });
}
async function ensureManagedServicesRunning(cfg, environmentId, timeoutMs) {
  section("Post-deploy service check");
  const search = await searchComposes(cfg, {
    environmentId,
    limit: 100,
    offset: 0
  });
  const byName = new Map((search?.items ?? []).map((c) => [c.name, c]));
  const failures = [];
  for (const app of APPS) {
    const compose = byName.get(app.name);
    if (!compose) {
      failures.push(`${app.name} (compose missing)`);
      continue;
    }
    const one = await getCompose(cfg, compose.composeId);
    const status = String(one?.composeStatus ?? "unknown");
    const hasDeployments = hasDeploymentHistory(one);
    if ((status === "running" || status === "done") && hasDeployments) {
      info(`${app.name}: ${status}`);
      continue;
    }
    warn(`${app.name}: ${status} (forcing compose.deploy)`);
    await triggerDeploy(cfg, compose.composeId);
    const result = await waitForManagedCompose(
      cfg,
      compose.composeId,
      Math.min(timeoutMs, 24e4)
    );
    if (!result.ok) {
      failures.push(`${app.name} (${result.reason})`);
    } else {
      info(`${app.name}: ${result.status}`);
    }
  }
  if (failures.length > 0) {
    fatal("Some services are not running after deploy.", `Failures: ${failures.join(", ")}`);
  }
}
function hasDeploymentHistory(composeOne) {
  if (Array.isArray(composeOne?.deployments) && composeOne.deployments.length > 0) return true;
  if (composeOne?.latestDeployment) return true;
  return false;
}
async function waitForManagedCompose(cfg, composeId, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const one = await getCompose(cfg, composeId);
    const status = String(one?.composeStatus ?? "unknown");
    const hasDeployments = hasDeploymentHistory(one);
    if ((status === "running" || status === "done") && hasDeployments) {
      return { ok: true, status };
    }
    if (status === "error" || status === "failed") {
      return { ok: false, status, reason: status };
    }
    await sleep2(3e3);
  }
  return { ok: false, status: "timeout", reason: "timeout waiting for compose status" };
}
function sleep2(ms) {
  return new Promise((r) => setTimeout(r, ms));
}
async function getLatestDeploymentSummary(cfg, composeId) {
  try {
    const latest = getLatestByCreatedAt(await listDeploymentsByCompose(cfg, composeId));
    if (!latest) return void 0;
    return {
      deploymentId: String(latest.deploymentId ?? "unknown"),
      status: String(latest.status ?? "unknown"),
      createdAt: latest.createdAt,
      errorMessage: latest.errorMessage,
      logPath: latest.logPath
    };
  } catch {
    return void 0;
  }
}
async function hasComposeServerBinding(cfg, composeId) {
  try {
    const one = await getCompose(cfg, composeId);
    return Boolean(one?.serverId);
  } catch {
    return false;
  }
}
function getLatestByCreatedAt(items) {
  if (!Array.isArray(items) || items.length === 0) return void 0;
  return [...items].sort((a, b) => toTs(b?.createdAt) - toTs(a?.createdAt))[0];
}
function toTs(raw) {
  const ts = Date.parse(String(raw ?? ""));
  return Number.isFinite(ts) ? ts : 0;
}
async function printServerDiagnosticSummary(cfg) {
  try {
    const servers = await listServers(cfg);
    if (!Array.isArray(servers) || servers.length === 0) return;
    const active = [...servers].filter((s) => String(s?.serverStatus ?? "") === "active");
    const selected = active[0] ?? servers[0];
    if (!selected?.serverId) return;
    info(
      `Server: ${selected.name ?? selected.serverId} (${selected.serverStatus ?? "unknown"}) ${selected.ipAddress ?? ""}`
    );
    const latest = getLatestByCreatedAt(
      await listDeploymentsByServer(cfg, String(selected.serverId))
    );
    if (!latest) return;
    info(
      `Latest server deployment: ${latest.status ?? "unknown"} (${latest.deploymentId ?? "unknown"})`
    );
    if (latest.errorMessage) warn(`Server deployment error: ${latest.errorMessage}`);
    if (latest.logPath) info(`Server log path: ${latest.logPath}`);
  } catch {
  }
}

// src/lib/prompts.ts
import * as p from "@clack/prompts";
function checkCancel(value) {
  if (p.isCancel(value)) {
    p.cancel("Cancelled.");
    process.exit(0);
  }
}
async function promptConfirm(message, initialValue = false) {
  const value = await p.confirm({ message, initialValue });
  checkCancel(value);
  return value;
}

// src/commands/destroy.ts
function registerDestroy(program2) {
  program2.command("destroy").description("Delete all compose services in the environment (irreversible)").option("--config <path>", "Path to .dokploy config file").option("--yes", "Skip confirmation prompt").option("--delete-volumes", "Also delete Docker volumes inside each compose").action(async (opts) => {
    let cfg;
    try {
      cfg = loadConfig({ config: opts.config });
    } catch (e) {
      fatal(e.message);
    }
    banner("destroy");
    section("\u26A0  DESTRUCTIVE OPERATION");
    let items;
    try {
      const search = await searchComposes(cfg, {
        environmentId: cfg.environmentId,
        limit: 100,
        offset: 0
      });
      items = search?.items ?? [];
    } catch (e) {
      fatal(`Failed to list composes: ${e.message}`);
    }
    if (items?.length === 0) {
      info("No compose services found in this environment.");
      return;
    }
    warn(`This will permanently delete ${items?.length} compose service(s):`);
    for (const c of items) {
      warn(`  \xB7 ${c.name} (${c.composeId}) \u2014 status: ${c.composeStatus}`);
    }
    console.log();
    if (!opts.yes) {
      const confirmed = await promptConfirm(
        "Are you sure you want to delete all these services?",
        false
      );
      if (!confirmed) {
        info("Destroy cancelled.");
        process.exit(0);
      }
    }
    section("Deleting composes");
    let failed = 0;
    for (const c of items) {
      try {
        const domains = await getDomainsByCompose(cfg, c.composeId);
        for (const d of domains ?? []) {
          try {
            await deleteDomain(cfg, d.domainId);
          } catch {
          }
        }
      } catch {
      }
      try {
        await deleteCompose(cfg, {
          composeId: c.composeId,
          deleteVolumes: Boolean(opts.deleteVolumes)
        });
        ok(`  \u2713 ${c.name}`);
      } catch (e) {
        const message = String(e?.message ?? e);
        if (!opts.deleteVolumes && message.includes("--deleteVolumes")) {
          warn(`  \u2717 ${c.name}: Dokploy CLI requires --deleteVolumes on this server version.`);
          warn(
            "    Re-run with --delete-volumes or upgrade Dokploy CLI/server for optional flag behavior."
          );
        } else {
          warn(`  \u2717 ${c.name}: ${message || "delete failed"}`);
        }
        failed++;
      }
    }
    console.log();
    if (failed > 0) {
      fatal(`${failed}/${items?.length} deletes failed. Check the Dokploy UI.`);
    } else {
      ok(`All ${items?.length} services deleted. Run \`epifly deploy\` to redeploy.`);
    }
  });
}

// src/commands/diff.ts
import { existsSync as existsSync2, readFileSync as readFileSync2 } from "fs";
import { resolve as resolve2 } from "path";
import pc2 from "picocolors";
function registerDiff(program2) {
  program2.command("diff").description("Diff local .env.production against live Dokploy Shared Env (project.env)").option("--config <path>", "Path to .dokploy config file").option(
    "--env-file <path>",
    "Path to local env file to compare (default: .env.production in repo root)"
  ).option("--show-secrets", "Show secret values instead of masking them").action(async (opts) => {
    let cfg;
    try {
      cfg = loadConfig({ config: opts.config });
    } catch (e) {
      fatal(e.message);
    }
    const envFile = opts.envFile ?? resolve2(cfg.repoRoot, ".env.production");
    if (!existsSync2(envFile)) {
      fatal(
        `Local env file not found: ${envFile}`,
        "Pass --env-file <path> to specify a different file."
      );
    }
    const local = parseDotenv(readFileSync2(envFile, "utf8"));
    let projectRecord;
    try {
      const envRecord = await getEnvironment(cfg, cfg.environmentId);
      projectRecord = await getProject(cfg, envRecord.projectId ?? envRecord.project?.projectId);
    } catch (e) {
      fatal(`Failed to fetch live env: ${e.message}`);
    }
    const remote = parseDotenv(projectRecord?.env ?? "");
    banner("diff");
    section(`Local: ${envFile} vs Remote: Dokploy project.env`);
    const allKeys = /* @__PURE__ */ new Set([...Object.keys(local), ...Object.keys(remote)]);
    let diffs = 0;
    for (const k of [...allKeys].sort()) {
      const l = local[k];
      const r = remote[k];
      const secret = isSecret(k) && !opts.showSecrets;
      if (!(k in local) && k in remote) {
        const val = secret ? "********" : r;
        console.log(`  ${pc2.dim("remote-only")} ${pc2.cyan(k.padEnd(36))} ${pc2.dim(val ?? "")}`);
        diffs++;
      } else if (k in local && !(k in remote)) {
        const val = secret ? "********" : l;
        console.log(
          `  ${pc2.yellow("local-only ")} ${pc2.yellow(k.padEnd(36))} ${pc2.dim(val ?? "")}`
        );
        diffs++;
      } else if (l !== r) {
        const lv = secret ? "********" : l;
        const rv = secret ? "********" : r;
        console.log(`  ${pc2.red("changed    ")} ${pc2.red(k.padEnd(36))}`);
        console.log(`    ${pc2.dim("local :")} ${lv ?? pc2.italic("(empty)")}`);
        console.log(`    ${pc2.dim("remote:")} ${rv ?? pc2.italic("(empty)")}`);
        diffs++;
      }
    }
    console.log();
    if (diffs === 0) {
      ok("No differences found");
    } else {
      warn(`${diffs} difference(s) found`);
      process.exitCode = 1;
    }
  });
}

// src/commands/doctor.ts
import pc3 from "picocolors";
async function runChecks(opts) {
  const results = [];
  let cfg;
  try {
    cfg = loadConfig({ config: opts.config });
    results.push({ label: "Config file loaded", passed: true, detail: opts.config ?? ".dokploy" });
  } catch (e) {
    results.push({ label: "Config file loaded", passed: false, detail: e.message });
    return results;
  }
  try {
    await getAllProjects(cfg);
    results.push({ label: `Dokploy API (${cfg.dokployUrl})`, passed: true });
  } catch (e) {
    results.push({ label: `Dokploy API (${cfg.dokployUrl})`, passed: false, detail: e.message });
    return results;
  }
  let envRecord;
  try {
    envRecord = await getEnvironment(cfg, cfg.environmentId);
    results.push({
      label: "Environment found",
      passed: true,
      detail: envRecord?.name ?? cfg.environmentId
    });
  } catch (e) {
    results.push({ label: "Environment found", passed: false, detail: e.message });
    return results;
  }
  const search = await searchComposes(cfg, {
    environmentId: cfg.environmentId,
    limit: 100,
    offset: 0
  });
  const existing = new Map((search?.items ?? []).map((c) => [c.name, c]));
  const expected = [...APPS.map((a) => a.name), "epifly-deploy"];
  for (const name of expected) {
    const compose = existing.get(name);
    results.push({
      label: `Compose '${name}' exists`,
      passed: existing.has(name),
      detail: existing.has(name) ? `composeId=${compose.composeId} status=${compose.composeStatus}` : "missing \u2014 run `epifly deploy` to create"
    });
  }
  try {
    const projectRecord = await getProject(
      cfg,
      envRecord.projectId ?? envRecord.project?.projectId
    );
    const env = projectRecord?.env ?? "";
    const missing = env.trim().length === 0;
    results.push({
      label: "Shared Env populated",
      passed: !missing,
      detail: missing ? "project.env is empty \u2014 run `epifly deploy`" : "ok"
    });
  } catch (e) {
    results.push({ label: "Shared Env populated", passed: false, detail: e.message });
  }
  return results;
}
function registerDoctor(program2) {
  program2.command("doctor").description("Run diagnostic checks against your Epifly environment").option("--config <path>", "Path to .dokploy config file").option("--json", "Output results as JSON").action(async (opts) => {
    if (!opts.json) banner("doctor");
    const results = await runChecks(opts);
    if (opts.json) {
      console.log(JSON.stringify(results, null, 2));
      const failed2 = results.filter((r) => !r.passed).length;
      process.exit(failed2 > 0 ? 1 : 0);
    }
    section("Checks");
    let failed = 0;
    for (const r of results) {
      const icon = r.passed ? pc3.green("\u2713") : pc3.red("\u2717");
      const label = r.passed ? pc3.white(r.label) : pc3.red(r.label);
      const detail = r.detail ? pc3.dim(` \u2014 ${r.detail}`) : "";
      console.log(`  ${icon} ${label}${detail}`);
      if (!r.passed) failed++;
    }
    console.log();
    if (failed === 0) {
      ok(`All ${results.length} checks passed`);
    } else {
      warn(`${failed}/${results.length} checks failed`);
      process.exit(1);
    }
  });
}

// src/commands/init.ts
import { writeFileSync as writeFileSync2 } from "fs";
import { resolve as resolve3 } from "path";
import * as p2 from "@clack/prompts";
function registerInit(program2) {
  program2.command("init").description("Interactive bootstrap wizard \u2014 create .dokploy config").option("--config <path>", "Config file to write (default: .dokploy in cwd)").action(async (opts) => {
    banner("init");
    p2.intro("Let's configure your Epifly environment.");
    const dokployUrl = await p2.text({
      message: "Dokploy URL",
      placeholder: "https://dokploy.example.com",
      validate: (v) => {
        if (!v.startsWith("http")) return "Must start with http:// or https://";
      }
    });
    if (p2.isCancel(dokployUrl)) {
      p2.cancel("Cancelled.");
      process.exit(0);
    }
    const apiKey = await p2.password({
      message: "Dokploy API key",
      validate: (v) => v.length < 8 ? "API key too short" : void 0
    });
    if (p2.isCancel(apiKey)) {
      p2.cancel("Cancelled.");
      process.exit(0);
    }
    const sp = p2.spinner();
    sp.start("Connecting to Dokploy API\u2026");
    const initCfg = { dokployUrl: dokployUrl.replace(/\/+$/, ""), apiKey };
    let environmentId;
    try {
      const envList = await listEnvironments(initCfg);
      sp.stop("Connected");
      if (envList.length === 0) {
        warn("No environments found. Create one in the Dokploy UI first.");
        process.exit(1);
      }
      const chosen = await p2.select({
        message: "Select environment",
        options: envList.map((e) => ({
          value: e.environmentId,
          label: e.name ?? e.environmentId,
          hint: e.description ?? ""
        }))
      });
      if (p2.isCancel(chosen)) {
        p2.cancel("Cancelled.");
        process.exit(0);
      }
      environmentId = chosen;
    } catch (e) {
      sp.stop("Failed");
      p2.log.error(`Cannot connect to Dokploy API: ${e.message}`);
      process.exit(1);
    }
    const appDomain = await p2.text({
      message: "APP_DOMAIN (e.g. epifly.prod.example.com)",
      placeholder: "epifly.prod.example.com",
      validate: (v) => {
        if (!v.includes(".")) return "Must be a valid domain";
      }
    });
    if (p2.isCancel(appDomain)) {
      p2.cancel("Cancelled.");
      process.exit(0);
    }
    const repoRoot = await p2.text({
      message: "Path to the conusai-platform repo root",
      defaultValue: process.cwd(),
      placeholder: process.cwd()
    });
    if (p2.isCancel(repoRoot)) {
      p2.cancel("Cancelled.");
      process.exit(0);
    }
    const dest = opts.config ?? resolve3(process.cwd(), ".dokploy");
    const cfg = {
      dokployUrl: dokployUrl.replace(/\/+$/, ""),
      apiKey,
      environmentId,
      appDomain,
      repoRoot
    };
    writeFileSync2(dest, `${JSON.stringify(cfg, null, 2)}
`, "utf8");
    section("Done");
    ok(`Config written to ${dest}`);
    p2.log.info(
      "Run `epifly status` to check your environment, or `epifly deploy` to trigger a deploy."
    );
    p2.outro("Happy deploying!");
  });
}

// src/commands/logs.ts
function registerLogs(program2) {
  program2.command("logs <app>").description("Stream recent log lines from a compose service").option("--config <path>", "Path to .dokploy config file").option("-n, --tail <n>", "Number of log lines to fetch", "200").option("--follow", "Poll for new lines every 3 s (Ctrl-C to stop)").action(async (appName, opts) => {
    let cfg;
    try {
      cfg = loadConfig({ config: opts.config });
    } catch (e) {
      fatal(e.message);
    }
    const validNames = [...APPS.map((a) => a.name), "epifly-deploy"];
    if (!validNames.includes(appName)) {
      fatal(`Unknown app: ${appName}`, `Valid apps: ${validNames.join(", ")}`);
    }
    banner(`logs \xB7 ${appName}`);
    let compose;
    try {
      const search = await searchComposes(cfg, {
        environmentId: cfg.environmentId,
        limit: 100,
        offset: 0
      });
      compose = (search?.items ?? []).find((x) => x.name === appName);
      if (!compose) {
        fatal(`Compose '${appName}' not found in this environment.`);
      }
    } catch (e) {
      fatal(`Failed to locate compose: ${e.message}`);
    }
    const composeId = String(compose.composeId);
    const composeAppName = String(compose.appName ?? "");
    const tail = normalizeTail(opts.tail);
    info(`Fetching logs for ${appName} (${composeId})`);
    if (opts.follow) {
      warn("--follow is not supported for Dokploy CLI log snapshots; showing latest logs once.");
    }
    if (!composeAppName) {
      warn("Compose has no appName, cannot resolve Docker containers through Dokploy CLI.");
      await printLatestDeploymentSummary(cfg, composeId, appName);
      return;
    }
    let containers = [];
    try {
      containers = await getServiceContainersByAppName(cfg, { appName: composeAppName }) ?? [];
      if (containers.length === 0) {
        containers = await getContainersByAppNameMatch(cfg, {
          appName: composeAppName,
          appType: "docker-compose"
        }) ?? [];
      }
    } catch (e) {
      warn(`Failed to list containers via Dokploy CLI: ${e.message}`);
    }
    if (containers.length === 0) {
      warn("No running containers found for this compose via Dokploy CLI.");
      await printLatestDeploymentSummary(cfg, composeId, appName);
      return;
    }
    for (const c of containers) {
      const containerId = String(c.containerId ?? c.id ?? "").trim();
      const containerName = String((c.name ?? c.containerName ?? containerId) || "unknown");
      if (!containerId) continue;
      console.log(`
=== container: ${containerName} (${containerId.slice(0, 12)}) ===`);
      try {
        const data = await readComposeLogs(cfg, { composeId, containerId, tail });
        printLogResult(data);
      } catch (e) {
        warn(`Failed to read logs for ${containerName}: ${e.message}`);
      }
    }
  });
}
function normalizeTail(raw) {
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : 200;
}
function printLogResult(data) {
  if (typeof data === "string") {
    process.stdout.write(data.endsWith("\n") ? data : `${data}
`);
    return;
  }
  if (Array.isArray(data)) {
    for (const line of data) {
      process.stdout.write(`${stringifyLine(line)}
`);
    }
    return;
  }
  if (Array.isArray(data?.logs)) {
    for (const line of data.logs) {
      process.stdout.write(`${stringifyLine(line)}
`);
    }
    return;
  }
  process.stdout.write(`${JSON.stringify(data, null, 2)}
`);
}
function stringifyLine(line) {
  if (typeof line === "string") return line;
  if (line?.message) return String(line.message);
  if (line?.log) return String(line.log);
  return JSON.stringify(line);
}
async function printLatestDeploymentSummary(cfg, composeId, composeName) {
  try {
    const one = await getCompose(cfg, composeId);
    const dep = getLatestByCreatedAt2(Array.isArray(one?.deployments) ? one.deployments : []);
    if (!dep) return;
    info(`Compose: ${composeName} (${composeId})`);
    info(`Latest deployment status: ${dep.status ?? "unknown"}`);
    if (dep.createdAt) info(`Created at: ${dep.createdAt}`);
    if (dep.title) info(`Title: ${dep.title}`);
    if (dep.errorMessage) warn(`Error message: ${dep.errorMessage}`);
    if (dep.logPath) info(`Log path reported by Dokploy: ${dep.logPath}`);
    if (!one?.serverId) {
      warn("Latest deployment has no serverId. Compose may not be bound to a Dokploy server.");
    }
    const servers = await listServers(cfg);
    const active = [...Array.isArray(servers) ? servers : []].filter(
      (s) => String(s?.serverStatus ?? "") === "active"
    );
    const selected = active[0] ?? (Array.isArray(servers) ? servers[0] : void 0);
    if (!selected?.serverId) return;
    info(
      `Server: ${selected.name ?? selected.serverId} (${selected.serverStatus ?? "unknown"}) ${selected.ipAddress ?? ""}`
    );
    const serverDep = getLatestByCreatedAt2(
      await listDeploymentsByServer(cfg, String(selected.serverId))
    );
    if (!serverDep) return;
    info(
      `Latest server deployment: ${serverDep.status ?? "unknown"} (${serverDep.deploymentId ?? "unknown"})`
    );
    if (serverDep.errorMessage) warn(`Server deployment error: ${serverDep.errorMessage}`);
    if (serverDep.logPath) info(`Server log path: ${serverDep.logPath}`);
  } catch {
  }
}
function getLatestByCreatedAt2(items) {
  if (!Array.isArray(items) || items.length === 0) return void 0;
  return [...items].sort((a, b) => toTs2(b?.createdAt) - toTs2(a?.createdAt))[0];
}
function toTs2(raw) {
  const ts = Date.parse(String(raw ?? ""));
  return Number.isFinite(ts) ? ts : 0;
}

// src/commands/secret.ts
import pc4 from "picocolors";

// ../../dokploy/lib/secrets.mjs
import { randomBytes, generateKeyPairSync } from "crypto";
var SECRETS = {
  POSTGRES_PASSWORD: () => randB64Url(30),
  // ~40 chars
  ZITADEL_MASTERKEY: () => randB64Url(24),
  // exactly 32 chars
  LAGO_SECRET_KEY_BASE: () => randHex(64),
  // 128 hex chars
  LAGO_ENCRYPTION_DET_KEY: () => randB64Url(24),
  LAGO_ENCRYPTION_SALT: () => randB64Url(24),
  LAGO_ENCRYPTION_KEY: () => randB64Url(24),
  LAGO_RSA_PRIVATE_KEY: () => base64(generateRsaPem(2048)),
  AWS_ACCESS_KEY_ID: () => "rfs_" + randUpperAlnum(15),
  AWS_SECRET_ACCESS_KEY: () => randB64Url(30),
  RUSTFS_IAM_ENC_KEY: () => randB64Url(24),
  RUSTFS_WEBHOOK_SECRET: () => randB64Url(32),
  UI_SESSION_KEY: () => randHex(32),
  // 64 hex chars (>32 bytes)
  PLATFORM_ADMIN_TOKEN: () => "pat_" + randB64Url(32)
};
var STATEFUL_SECRETS = {
  POSTGRES_PASSWORD: "conusai_postgres_data",
  LAGO_SECRET_KEY_BASE: "conusai_postgres_data",
  LAGO_ENCRYPTION_DET_KEY: "conusai_postgres_data",
  LAGO_ENCRYPTION_SALT: "conusai_postgres_data",
  LAGO_ENCRYPTION_KEY: "conusai_postgres_data",
  LAGO_RSA_PRIVATE_KEY: "conusai_postgres_data",
  RUSTFS_IAM_ENC_KEY: "conusai_rustfs_data",
  AWS_ACCESS_KEY_ID: "conusai_rustfs_data",
  AWS_SECRET_ACCESS_KEY: "conusai_rustfs_data",
  ZITADEL_MASTERKEY: "conusai_postgres_data"
};
function randB64Url(bytes) {
  return randomBytes(bytes).toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function randHex(bytes) {
  return randomBytes(bytes).toString("hex");
}
function randUpperAlnum(n) {
  const charset = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  const buf = randomBytes(n);
  let s = "";
  for (let i = 0; i < n; i++) s += charset[buf[i] % charset.length];
  return s;
}
function base64(s) {
  return Buffer.from(s, "utf8").toString("base64");
}
function generateRsaPem(modulusLength) {
  return generateKeyPairSync("rsa", { modulusLength }).privateKey.export({
    type: "pkcs8",
    format: "pem"
  });
}

// src/commands/secret.ts
function registerSecret(program2) {
  const secret = program2.command("secret").description("Manage Shared Env secrets");
  secret.command("list").description("List all Shared Env keys (values masked for secrets)").option("--config <path>", "Path to .dokploy config file").option("--show", "Show secret values in plaintext").action(async (opts) => {
    let cfg;
    try {
      cfg = loadConfig({ config: opts.config });
    } catch (e) {
      fatal(e.message);
    }
    const envRecord = await getEnvironment(cfg, cfg.environmentId);
    const projectRecord = await getProject(
      cfg,
      envRecord.projectId ?? envRecord.project?.projectId
    );
    const current = parseDotenv(projectRecord?.env ?? "");
    section(`Shared Env (${Object.keys(current).length} keys)`);
    for (const [k, v] of Object.entries(current).sort(([a], [b]) => a.localeCompare(b))) {
      const masked = isSecret(k) && !opts.show ? pc4.dim("********") : pc4.green(v || pc4.italic("(empty)"));
      const stateful = k in STATEFUL_SECRETS ? pc4.yellow(" [stateful]") : "";
      const managed = k in SECRETS ? pc4.blue(" [managed]") : "";
      console.log(`  ${k.padEnd(38)} ${masked}${stateful}${managed}`);
    }
  });
  secret.command("get <key>").description("Print the current value of a single key").option("--config <path>", "Path to .dokploy config file").action(async (key, opts) => {
    let cfg;
    try {
      cfg = loadConfig({ config: opts.config });
    } catch (e) {
      fatal(e.message);
    }
    const envRecord = await getEnvironment(cfg, cfg.environmentId);
    const projectRecord = await getProject(
      cfg,
      envRecord.projectId ?? envRecord.project?.projectId
    );
    const current = parseDotenv(projectRecord?.env ?? "");
    if (!(key in current)) {
      err(`Key not found: ${key}`);
      process.exit(1);
    }
    process.stdout.write(`${current[key]}
`);
  });
  secret.command("rotate <key>").description("Regenerate a managed secret in Shared Env").option("--config <path>", "Path to .dokploy config file").option("--yes", "Skip confirmation prompt").action(async (key, opts) => {
    let cfg;
    try {
      cfg = loadConfig({ config: opts.config });
    } catch (e) {
      fatal(e.message);
    }
    if (!(key in SECRETS)) {
      fatal(
        `'${key}' is not a managed secret.`,
        `Managed secrets: ${Object.keys(SECRETS).join(", ")}`
      );
    }
    if (key in STATEFUL_SECRETS) {
      warn(
        `'${key}' is a STATEFUL secret bound to volume '${STATEFUL_SECRETS[key]}'. Rotating it will CORRUPT the data in that volume!`
      );
      if (!opts.yes) {
        const confirmed = await promptConfirm(
          "Are you absolutely sure you want to rotate this stateful secret?",
          false
        );
        if (!confirmed) {
          info("Rotation cancelled.");
          process.exit(0);
        }
      }
    }
    banner("secret rotate");
    const envRecord = await getEnvironment(cfg, cfg.environmentId);
    const projectId = envRecord.projectId ?? envRecord.project?.projectId;
    const projectRecord = await getProject(cfg, projectId);
    const current = parseDotenv(projectRecord?.env ?? "");
    const newValue = SECRETS[key]();
    const merged = { ...current, [key]: newValue };
    const envText = renderDotenv(merged, projectRecord?.env ?? "");
    await updateProject(cfg, { projectId, env: envText });
    ok(`Rotated ${key}`);
    info(`New value: ${isSecret(key) ? "******** (masked)" : newValue}`);
    info("Run `epifly deploy` to apply the new secret to all services.");
  });
}

// src/commands/status.ts
var STATUS_COLOR = {
  done: "ok",
  running: "ok",
  error: "error",
  failed: "error",
  idle: "idle",
  queued: "queued"
};
function registerStatus(program2) {
  program2.command("status").description("Show deploy status of all compose services in the environment").option("--config <path>", "Path to .dokploy config file").option("--json", "Output as JSON").action(async (opts) => {
    let cfg;
    try {
      cfg = loadConfig({ config: opts.config });
    } catch (e) {
      fatal(e.message);
    }
    let search;
    try {
      search = await searchComposes(cfg, {
        environmentId: cfg.environmentId,
        limit: 100,
        offset: 0
      });
    } catch (e) {
      fatal(`Failed to fetch composes: ${e.message}`);
    }
    const items = search?.items ?? [];
    if (opts.json) {
      console.log(JSON.stringify(items, null, 2));
      return;
    }
    banner("status");
    section(`${cfg.appDomain} (${items.length} services)`);
    if (items.length === 0) {
      info("No compose services found in this environment.");
      return;
    }
    const rows = items.map((c) => [
      c.name ?? c.composeId,
      c.composeStatus ?? "unknown",
      STATUS_COLOR[c.composeStatus] ?? c.composeStatus
    ]);
    table(rows);
  });
}

// ../../dokploy/lib/verify.mjs
function buildVerifyChecks(appDomain) {
  return [
    {
      label: "web (root)",
      url: `https://${appDomain}`,
      expectStatus: [200, 301, 302, 307, 308]
    },
    {
      label: "zitadel OIDC discovery",
      url: `https://auth.${appDomain}/.well-known/openid-configuration`,
      expectStatus: [200],
      expectJsonKey: "issuer"
    },
    {
      label: "lago health",
      url: `https://billing.${appDomain}/health`,
      expectStatus: [200, 301, 302]
    },
    {
      label: "rustfs S3 (anon)",
      url: `https://s3.${appDomain}/minio/health/live`,
      expectStatus: [200, 204, 403]
    },
    {
      label: "rustfs console",
      url: `https://s3-console.${appDomain}`,
      expectStatus: [200, 301, 302, 307, 308, 401, 403]
    },
    {
      label: "jaeger UI",
      url: `https://traces.${appDomain}`,
      expectStatus: [200, 301, 302, 307, 308]
    },
    {
      label: "gateway api health",
      url: `https://api.${appDomain}/health`,
      expectStatus: [200, 404]
      // 404 acceptable until route is added
    }
  ];
}
async function runCheckDetailed({ url, expectStatus, expectJsonKey }) {
  const ac = new AbortController();
  const t = setTimeout(() => ac.abort(), 15e3);
  try {
    const res = await fetch(url, { redirect: "manual", signal: ac.signal });
    if (!expectStatus.includes(res.status)) {
      return {
        ok: false,
        status: res.status,
        error: `unexpected status ${res.status}, expected one of [${expectStatus.join(", ")}]`
      };
    }
    if (expectJsonKey) {
      const body = await res.json().catch(() => null);
      if (!body || !(expectJsonKey in body)) {
        return {
          ok: false,
          status: res.status,
          error: "response JSON missing expected key",
          missingJsonKey: expectJsonKey
        };
      }
    }
    return { ok: true, status: res.status };
  } catch (err3) {
    if (err3.name === "AbortError") {
      return { ok: false, error: `${url} timed out` };
    }
    return { ok: false, error: formatFetchError(err3) };
  } finally {
    clearTimeout(t);
  }
}
function formatFetchError(err3) {
  const anyErr = (
    /** @type {any} */
    err3
  );
  const code = anyErr?.cause?.code || anyErr?.code;
  const msg = anyErr?.message || String(err3);
  if (code === "UNABLE_TO_VERIFY_LEAF_SIGNATURE" || code === "CERT_HAS_EXPIRED") {
    return `TLS certificate verification failed (${code})`;
  }
  if (code === "DEPTH_ZERO_SELF_SIGNED_CERT" || code === "SELF_SIGNED_CERT_IN_CHAIN") {
    return `TLS certificate chain is not trusted (${code})`;
  }
  if (code === "ENOTFOUND") {
    return `DNS lookup failed (${code})`;
  }
  if (code === "ECONNREFUSED" || code === "ECONNRESET") {
    return `network connection failed (${code})`;
  }
  return code ? `${msg} (${code})` : msg;
}

// src/commands/verify.ts
function registerVerify(program2) {
  program2.command("verify").description("Run HTTPS smoke-tests against a live environment").option("-d, --domain <domain>", "APP_DOMAIN to verify (overrides config)").option("--config <path>", "Path to .dokploy config file").action(async (opts) => {
    const partial = readPartialConfig({ config: opts.config, appDomain: opts.domain });
    const appDomain = partial.appDomain;
    if (!appDomain)
      fatal("No APP_DOMAIN. Pass --domain or set it in .dokploy / APP_DOMAIN env var.");
    banner("verify");
    section(`Checking ${appDomain}`);
    const checks = buildVerifyChecks(appDomain);
    const failures = [];
    for (const check of checks) {
      const result = await runCheckDetailed(check);
      const label = check.label.padEnd(52);
      if (result.ok) {
        const status = result.status !== void 0 ? ` [${result.status}]` : "";
        ok(`${label} ${check.url}${status}`);
      } else {
        const status = result.status !== void 0 ? ` [${result.status}]` : "";
        const reason = result.error ?? "check failed";
        const keyHint = result.missingJsonKey ? ` (missing JSON key: ${result.missingJsonKey})` : "";
        err(`${label} ${check.url}${status} (${reason})${keyHint}`);
        failures.push(check);
      }
    }
    console.log();
    if (failures.length > 0) {
      warn(`${failures.length}/${checks.length} checks failed`);
      process.exit(1);
    } else {
      ok(`All ${checks.length} checks passed`);
    }
  });
}

// src/lib/ssh.ts
import { spawnSync } from "child_process";
function runOverSsh(opts, command) {
  const args = [];
  if (opts.jumpHost) args.push("-J", opts.jumpHost);
  if (opts.port) args.push("-p", String(opts.port));
  if (opts.identityFile) args.push("-i", opts.identityFile);
  args.push("-o", "BatchMode=yes");
  args.push("-o", "StrictHostKeyChecking=accept-new");
  args.push(opts.host, command);
  const result = spawnSync("ssh", args, { stdio: "inherit" });
  if (result.error) throw new Error(`ssh failed to start: ${result.error.message}`);
  if (result.status !== 0) {
    throw new Error(`Remote command exited with code ${result.status}`);
  }
}

// src/commands/wipe.ts
var VOLUME_GROUPS = {
  postgres: ["conusai_postgres_data"],
  redis: ["conusai_redis_data"],
  qdrant: ["conusai_qdrant_data"],
  rustfs: ["conusai_rustfs_data", "conusai_redb_data"],
  lago: ["conusai_lago_storage_data"]
};
function registerWipe(program2) {
  program2.command("wipe").description("Destructively wipe named Docker volumes on the Dokploy host via SSH").requiredOption("--host <user@host>", "SSH target for the Dokploy host").option("--config <path>", "Path to .dokploy config file").option("--all", "Wipe ALL managed volumes").option("--postgres", "Wipe Postgres data volume").option("--redis", "Wipe Redis data volume").option("--qdrant", "Wipe Qdrant data volume").option("--rustfs", "Wipe RustFS + Redb volumes").option("--lago", "Wipe Lago storage volume").option("--port <n>", "SSH port (default 22)").option("--identity <file>", "SSH identity file").option("--yes", "Skip confirmation prompt").option("--no-backup", "Skip Postgres pg_dump before wiping").action(async (opts) => {
    const volumes = [];
    if (opts.all) {
      volumes.push(...EXTERNAL_VOLUMES);
    } else {
      for (const [key, vols] of Object.entries(VOLUME_GROUPS)) {
        if (opts[key]) volumes.push(...vols);
      }
    }
    if (volumes.length === 0) {
      fatal(
        "No volumes selected.",
        "Use --all, --postgres, --redis, --qdrant, --rustfs, or --lago."
      );
    }
    banner("wipe");
    section("\u26A0  DESTRUCTIVE OPERATION");
    warn("This will PERMANENTLY DELETE the following Docker volumes:");
    for (const v of volumes) warn(`  \xB7 ${v}`);
    console.log();
    if (!opts.yes) {
      const confirmed = await promptConfirm(
        "Type 'yes' to confirm you understand this will destroy all data in these volumes.",
        false
      );
      if (!confirmed) {
        info("Wipe cancelled.");
        process.exit(0);
      }
    }
    const flags = [];
    if (opts.all) flags.push("--all");
    if (opts.postgres) flags.push("--postgres");
    if (opts.redis) flags.push("--redis");
    if (opts.qdrant) flags.push("--qdrant");
    if (opts.rustfs) flags.push("--rustfs");
    if (opts.lago) flags.push("--lago");
    if (opts.noBackup) flags.push("--no-backup");
    flags.push("--yes");
    const remoteScript = "/opt/epifly/scripts/wipe-volumes.sh";
    const cmd = `bash ${remoteScript} ${flags.join(" ")}`;
    info(`Connecting to ${opts.host}\u2026`);
    info(`Running: ${cmd}`);
    console.log();
    try {
      runOverSsh(
        {
          host: opts.host,
          port: opts.port ? Number(opts.port) : void 0,
          identityFile: opts.identity
        },
        cmd
      );
    } catch (e) {
      fatal(`Wipe failed: ${e.message}`);
    }
  });
}

// src/cli.ts
var program = new Command("epifly").version("0.1.0").description("Operator CLI for Epifly \u2014 manage Dokploy-hosted stacks");
registerInit(program);
registerDeploy(program);
registerDestroy(program);
registerLogs(program);
registerVerify(program);
registerSecret(program);
registerStatus(program);
registerDiff(program);
registerWipe(program);
registerDoctor(program);
program.parseAsync(process.argv).catch((err3) => {
  console.error(err3 instanceof Error ? err3.message : err3);
  process.exit(1);
});
//# sourceMappingURL=epifly.mjs.map