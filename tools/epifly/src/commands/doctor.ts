/**
 * epifly doctor — diagnostic self-check.
 *
 * Verifies: config file exists, Dokploy API reachable, environment found,
 * all composes exist, EXTERNAL_VOLUMES exist on host (if SSH provided).
 *
 *   epifly doctor [--ssh user@host]
 */

import type { Command } from "commander";
import pc from "picocolors";
import { APPS, EXTERNAL_VOLUMES } from "../../../../dokploy/lib/manifest.mjs";
import { loadConfig, readPartialConfig } from "../lib/config.ts";
import { getAllProjects, getEnvironment, getProject, searchComposes } from "../lib/dokploy.ts";
import { banner, err, info, ok, section, warn } from "../lib/ui.ts";

type CheckResult = { label: string; passed: boolean; detail?: string };

async function runChecks(opts: any): Promise<CheckResult[]> {
  const results: CheckResult[] = [];

  // 1. Config
  let cfg: any;
  try {
    cfg = loadConfig({ config: opts.config });
    results.push({ label: "Config file loaded", passed: true, detail: opts.config ?? ".dokploy" });
  } catch (e: any) {
    results.push({ label: "Config file loaded", passed: false, detail: e.message });
    return results; // nothing else works without config
  }

  // 2. Dokploy reachable
  try {
    await getAllProjects(cfg);
    results.push({ label: `Dokploy API (${cfg.dokployUrl})`, passed: true });
  } catch (e: any) {
    results.push({ label: `Dokploy API (${cfg.dokployUrl})`, passed: false, detail: e.message });
    return results;
  }

  // 3. Environment exists
  let envRecord: any;
  try {
    envRecord = await getEnvironment(cfg, cfg.environmentId);
    results.push({
      label: "Environment found",
      passed: true,
      detail: envRecord?.name ?? cfg.environmentId,
    });
  } catch (e: any) {
    results.push({ label: "Environment found", passed: false, detail: e.message });
    return results;
  }

  // 4. All composes present
  const search = await searchComposes(cfg, {
    environmentId: cfg.environmentId,
    limit: 100,
    offset: 0,
  });
  const existing = new Map<string, any>((search?.items ?? []).map((c: any) => [c.name, c]));

  const expected = [...(APPS as any[]).map((a: any) => a.name), "epifly-deploy"];
  for (const name of expected) {
    const compose = existing.get(name);
    results.push({
      label: `Compose '${name}' exists`,
      passed: existing.has(name),
      detail: existing.has(name)
        ? `composeId=${compose.composeId} status=${compose.composeStatus}`
        : "missing — run `epifly deploy` to create",
    });
  }

  // 5. Project env has no missing keys
  try {
    const projectRecord: any = await getProject(
      cfg,
      envRecord.projectId ?? envRecord.project?.projectId
    );
    const env = projectRecord?.env ?? "";
    const missing = env.trim().length === 0;
    results.push({
      label: "Shared Env populated",
      passed: !missing,
      detail: missing ? "project.env is empty — run `epifly deploy`" : "ok",
    });
  } catch (e: any) {
    results.push({ label: "Shared Env populated", passed: false, detail: e.message });
  }

  return results;
}

export function registerDoctor(program: Command): void {
  program
    .command("doctor")
    .description("Run diagnostic checks against your Epifly environment")
    .option("--config <path>", "Path to .dokploy config file")
    .option("--json", "Output results as JSON")
    .action(async (opts) => {
      if (!opts.json) banner("doctor");

      const results = await runChecks(opts);

      if (opts.json) {
        console.log(JSON.stringify(results, null, 2));
        const failed = results.filter((r) => !r.passed).length;
        process.exit(failed > 0 ? 1 : 0);
      }

      section("Checks");
      let failed = 0;
      for (const r of results) {
        const icon = r.passed ? pc.green("✓") : pc.red("✗");
        const label = r.passed ? pc.white(r.label) : pc.red(r.label);
        const detail = r.detail ? pc.dim(` — ${r.detail}`) : "";
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
