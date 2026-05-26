/**
 * epifly logs <app> — fetch recent container logs for a compose service
 * using Dokploy CLI only (no SSH fallback).
 */

import type { Command } from "commander";
import { APPS } from "../../../../dokploy/lib/manifest.mjs";
import { loadConfig } from "../lib/config.ts";
import {
  getCompose,
  getContainersByAppNameMatch,
  listDeploymentsByServer,
  listServers,
  getServiceContainersByAppName,
  readComposeLogs,
  searchComposes,
} from "../lib/dokploy.ts";
import { banner, fatal, info, warn } from "../lib/ui.ts";

export function registerLogs(program: Command): void {
  program
    .command("logs <app>")
    .description("Stream recent log lines from a compose service")
    .option("--config <path>", "Path to .dokploy config file")
    .option("-n, --tail <n>", "Number of log lines to fetch", "200")
    .option("--follow", "Poll for new lines every 3 s (Ctrl-C to stop)")
    .action(async (appName: string, opts) => {
      let cfg;
      try {
        cfg = loadConfig({ config: opts.config });
      } catch (e: any) {
        fatal(e.message);
      }

      const validNames = [...APPS.map((a: any) => a.name), "epifly-deploy"];
      if (!validNames.includes(appName)) {
        fatal(`Unknown app: ${appName}`, `Valid apps: ${validNames.join(", ")}`);
      }

      banner(`logs · ${appName}`);

      let compose: any;
      try {
        const search = await searchComposes(cfg, {
          environmentId: cfg.environmentId,
          limit: 100,
          offset: 0,
        });
        compose = (search?.items ?? []).find((x: any) => x.name === appName);
        if (!compose) {
          fatal(`Compose '${appName}' not found in this environment.`);
        }
      } catch (e: any) {
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

      let containers: any[] = [];
      try {
        containers = await getServiceContainersByAppName(cfg, { appName: composeAppName }) ?? [];
        if (containers.length === 0) {
          containers = await getContainersByAppNameMatch(cfg, {
            appName: composeAppName,
            appType: "docker-compose",
          }) ?? [];
        }
      } catch (e: any) {
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

        console.log(`\n=== container: ${containerName} (${containerId.slice(0, 12)}) ===`);
        try {
          const data = await readComposeLogs(cfg, { composeId, containerId, tail });
          printLogResult(data);
        } catch (e: any) {
          warn(`Failed to read logs for ${containerName}: ${e.message}`);
        }
      }
    });
}

function normalizeTail(raw: string): number {
  const n = Number(raw);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : 200;
}

function printLogResult(data: any): void {
  if (typeof data === "string") {
    process.stdout.write(data.endsWith("\n") ? data : `${data}\n`);
    return;
  }

  if (Array.isArray(data)) {
    for (const line of data) {
      process.stdout.write(`${stringifyLine(line)}\n`);
    }
    return;
  }

  if (Array.isArray(data?.logs)) {
    for (const line of data.logs) {
      process.stdout.write(`${stringifyLine(line)}\n`);
    }
    return;
  }

  process.stdout.write(`${JSON.stringify(data, null, 2)}\n`);
}

function stringifyLine(line: any): string {
  if (typeof line === "string") return line;
  if (line?.message) return String(line.message);
  if (line?.log) return String(line.log);
  return JSON.stringify(line);
}

async function printLatestDeploymentSummary(cfg: any, composeId: string, composeName: string): Promise<void> {
  try {
    const one: any = await getCompose(cfg, composeId);
    const dep = getLatestByCreatedAt(Array.isArray(one?.deployments) ? one.deployments : []);
    if (!dep) return;

    info(`Compose: ${composeName} (${composeId})`);
    info(`Latest deployment status: ${dep.status ?? "unknown"}`);
    if (dep.createdAt) info(`Created at: ${dep.createdAt}`);
    if (dep.title) info(`Title: ${dep.title}`);
    if (dep.errorMessage) warn(`Error message: ${dep.errorMessage}`);
    if (dep.logPath) info(`Log path reported by Dokploy: ${dep.logPath}`);
    if (!one?.serverId) {
      warn(
        "Latest deployment has no serverId. Compose may not be bound to a Dokploy server.",
      );
    }

    const servers = await listServers(cfg);
    const active = [...(Array.isArray(servers) ? servers : [])].filter(
      (s: any) => String(s?.serverStatus ?? "") === "active",
    );
    const selected = active[0] ?? (Array.isArray(servers) ? servers[0] : undefined);
    if (!selected?.serverId) return;

    info(
      `Server: ${selected.name ?? selected.serverId} (${selected.serverStatus ?? "unknown"}) ${selected.ipAddress ?? ""}`,
    );

    const serverDep = getLatestByCreatedAt(await listDeploymentsByServer(cfg, String(selected.serverId)));
    if (!serverDep) return;
    info(
      `Latest server deployment: ${serverDep.status ?? "unknown"} (${serverDep.deploymentId ?? "unknown"})`,
    );
    if (serverDep.errorMessage) warn(`Server deployment error: ${serverDep.errorMessage}`);
    if (serverDep.logPath) info(`Server log path: ${serverDep.logPath}`);
  } catch {
    // Best-effort diagnostics only.
  }
}

function getLatestByCreatedAt(items: any[]): any | undefined {
  if (!Array.isArray(items) || items.length === 0) return undefined;
  return [...items].sort((a, b) => toTs(b?.createdAt) - toTs(a?.createdAt))[0];
}

function toTs(raw: unknown): number {
  const ts = Date.parse(String(raw ?? ""));
  return Number.isFinite(ts) ? ts : 0;
}
