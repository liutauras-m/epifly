/**
 * epifly deploy — trigger epifly-deploy orchestrator and tail logs.
 *
 *   epifly deploy [--dry-run] [--phase <name>] [--skip-verify]
 */

import type { Command } from "commander";
import { parseDotenv, renderDotenv } from "../../../../dokploy/lib/dotenv.mjs";
import { APPS } from "../../../../dokploy/lib/manifest.mjs";
import { loadConfig } from "../lib/config.ts";
import {
  getCompose,
  getServerCount,
  listDeploymentsByCompose,
  listDeploymentsByServer,
  listServers,
  searchComposes,
  triggerDeploy,
  updateCompose,
} from "../lib/dokploy.ts";
import { tailDeployLogs } from "../lib/log-tail.ts";
import { banner, fatal, info, ok, section, warn } from "../lib/ui.ts";

const ALLOWED_PHASES = ["volumes", "env", "composes", "domains", "deploys", "verify"] as const;

export function registerDeploy(program: Command): void {
  program
    .command("deploy")
    .description("Trigger the epifly-deploy orchestrator and stream logs")
    .option("--config <path>", "Path to .dokploy config file")
    .option("--dry-run", "Pass DEPLOY_DRY_RUN=true to the orchestrator")
    .option("--phase <name>", "Run a single phase (volumes|env|composes|domains|deploys|verify)")
    .option("--skip-verify", "Skip Phase 5 verify")
    .option("--timeout <secs>", "Per-deploy timeout in seconds", "600")
    .action(async (opts) => {
      let cfg;
      try {
        cfg = loadConfig({ config: opts.config });
      } catch (e: any) {
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
      } catch (e: any) {
        warn(`Could not verify Dokploy server count: ${e.message}`);
      }

      // Find epifly-deploy compose ID
      let composeId: string;
      try {
        const search = await searchComposes(cfg, {
          environmentId: cfg.environmentId,
          limit: 100,
          offset: 0,
        });
        const self = (search?.items ?? []).find((c: any) => c.name === "epifly-deploy");
        if (!self) {
          fatal(
            "No 'epifly-deploy' compose found in this environment.",
            "Run `epifly init` or create it manually in the Dokploy UI."
          );
        }
        composeId = self.composeId;
      } catch (e: any) {
        fatal(`Failed to locate epifly-deploy compose: ${e.message}`);
      }

      // Build per-compose env overrides for this deploy
      const envOverrides: Record<string, string> = {};
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
          const compose: any = await getCompose(cfg, composeId!);
          priorComposeEnv = compose?.env ?? "";
          const merged = {
            ...parseDotenv(priorComposeEnv),
            ...envOverrides,
          };
          const nextEnv = renderDotenv(merged, priorComposeEnv);
          await updateCompose(cfg, { composeId: composeId!, env: nextEnv });
          appliedTemporaryOverrides = true;
          info("Applied temporary compose env overrides for this deploy.");
        } catch (e: any) {
          fatal(`Failed to apply temporary deploy overrides: ${e.message}`);
        }
      }

      section("Triggering deploy");
      info(`Compose: epifly-deploy (${composeId!})`);

      let finalStatus = "unknown";
      try {
        const trigger = await triggerDeploy(cfg, composeId!);
        if (trigger?.success === false) {
          fatal(`Dokploy rejected deploy trigger: ${trigger?.message || "unknown error"}`);
        }
        ok("Deploy triggered — tailing logs…");
        console.log();

        const timeoutMs = Number(opts.timeout) * 1000;
        finalStatus = await tailDeployLogs(cfg, composeId!, timeoutMs);
      } catch (e: any) {
        fatal(`Failed to trigger deploy: ${e.message}`);
      } finally {
        if (appliedTemporaryOverrides) {
          try {
            await updateCompose(cfg, { composeId: composeId!, env: priorComposeEnv });
            info("Restored compose env after deploy.");
          } catch (e: any) {
            warn(`Failed to restore compose env overrides: ${e.message}`);
            warn("Please verify DEPLOY_* variables in the epifly-deploy compose env.");
          }
        }
      }

      console.log();
      if (finalStatus === "done") {
        await ensureManagedServicesRunning(cfg, cfg.environmentId, Number(opts.timeout) * 1000);
        ok("Deploy completed successfully");
      } else if (finalStatus === "queue_stalled") {
        const latest = await getLatestDeploymentSummary(cfg, composeId!);
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
        const latest = await getLatestDeploymentSummary(cfg, composeId!);
        if (latest) {
          info(`Latest deployment: ${latest.status} (${latest.deploymentId})`);
          if (latest.errorMessage) warn(`Error: ${latest.errorMessage}`);
          if (latest.logPath) info(`Log path: ${latest.logPath}`);
          if (!(await hasComposeServerBinding(cfg, composeId!))) {
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

async function ensureManagedServicesRunning(
  cfg: any,
  environmentId: string,
  timeoutMs: number
): Promise<void> {
  section("Post-deploy service check");

  const search = await searchComposes(cfg, {
    environmentId,
    limit: 100,
    offset: 0,
  });
  const byName = new Map<string, any>((search?.items ?? []).map((c: any) => [c.name, c]));

  const failures: string[] = [];
  for (const app of APPS) {
    const compose = byName.get(app.name);
    if (!compose) {
      failures.push(`${app.name} (compose missing)`);
      continue;
    }

    const one: any = await getCompose(cfg, compose.composeId);
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
      Math.min(timeoutMs, 240_000)
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

function hasDeploymentHistory(composeOne: any): boolean {
  if (Array.isArray(composeOne?.deployments) && composeOne.deployments.length > 0) return true;
  if (composeOne?.latestDeployment) return true;
  return false;
}

async function waitForManagedCompose(
  cfg: any,
  composeId: string,
  timeoutMs: number
): Promise<{ ok: boolean; status: string; reason?: string }> {
  const start = Date.now();

  while (Date.now() - start < timeoutMs) {
    const one: any = await getCompose(cfg, composeId);
    const status = String(one?.composeStatus ?? "unknown");
    const hasDeployments = hasDeploymentHistory(one);

    if ((status === "running" || status === "done") && hasDeployments) {
      return { ok: true, status };
    }
    if (status === "error" || status === "failed") {
      return { ok: false, status, reason: status };
    }

    await sleep(3000);
  }

  return { ok: false, status: "timeout", reason: "timeout waiting for compose status" };
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

async function getLatestDeploymentSummary(
  cfg: any,
  composeId: string
): Promise<
  | {
      deploymentId: string;
      status: string;
      createdAt?: string;
      errorMessage?: string;
      logPath?: string;
    }
  | undefined
> {
  try {
    const latest = getLatestByCreatedAt(await listDeploymentsByCompose(cfg, composeId));
    if (!latest) return undefined;
    return {
      deploymentId: String(latest.deploymentId ?? "unknown"),
      status: String(latest.status ?? "unknown"),
      createdAt: latest.createdAt,
      errorMessage: latest.errorMessage,
      logPath: latest.logPath,
    };
  } catch {
    return undefined;
  }
}

async function hasComposeServerBinding(cfg: any, composeId: string): Promise<boolean> {
  try {
    const one = await getCompose(cfg, composeId);
    return Boolean(one?.serverId);
  } catch {
    return false;
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

async function printServerDiagnosticSummary(cfg: any): Promise<void> {
  try {
    const servers = await listServers(cfg);
    if (!Array.isArray(servers) || servers.length === 0) return;

    const active = [...servers].filter((s: any) => String(s?.serverStatus ?? "") === "active");
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
    // Best-effort diagnostics only.
  }
}
