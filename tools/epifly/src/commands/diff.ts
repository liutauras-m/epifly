/**
 * epifly diff — compare local .env.production with the live Dokploy Shared Env.
 *
 *   epifly diff [--env-file <path>]
 */

import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { Command } from "commander";
import pc from "picocolors";
import { parseDotenv } from "../../../../dokploy/lib/dotenv.mjs";
import { isSecret } from "../../../../dokploy/lib/dotenv.mjs";
import { loadConfig } from "../lib/config.ts";
import { getEnvironment, getProject } from "../lib/dokploy.ts";
import { banner, fatal, info, ok, section, warn } from "../lib/ui.ts";

export function registerDiff(program: Command): void {
  program
    .command("diff")
    .description("Diff local .env.production against live Dokploy Shared Env (project.env)")
    .option("--config <path>", "Path to .dokploy config file")
    .option(
      "--env-file <path>",
      "Path to local env file to compare (default: .env.production in repo root)"
    )
    .option("--show-secrets", "Show secret values instead of masking them")
    .action(async (opts) => {
      let cfg;
      try {
        cfg = loadConfig({ config: opts.config });
      } catch (e: any) {
        fatal(e.message);
      }

      const envFile = opts.envFile ?? resolve(cfg.repoRoot, ".env.production");

      if (!existsSync(envFile)) {
        fatal(
          `Local env file not found: ${envFile}`,
          "Pass --env-file <path> to specify a different file."
        );
      }

      const local = parseDotenv(readFileSync(envFile, "utf8"));

      let projectRecord: any;
      try {
        const envRecord = await getEnvironment(cfg, cfg.environmentId);
        projectRecord = await getProject(cfg, envRecord.projectId ?? envRecord.project?.projectId);
      } catch (e: any) {
        fatal(`Failed to fetch live env: ${e.message}`);
      }

      const remote = parseDotenv(projectRecord?.env ?? "");

      banner("diff");
      section(`Local: ${envFile} vs Remote: Dokploy project.env`);

      const allKeys = new Set([...Object.keys(local), ...Object.keys(remote)]);
      let diffs = 0;

      for (const k of [...allKeys].sort()) {
        const l = local[k];
        const r = remote[k];
        const secret = isSecret(k) && !opts.showSecrets;

        if (!(k in local) && k in remote) {
          // Only in remote
          const val = secret ? "********" : r;
          console.log(`  ${pc.dim("remote-only")} ${pc.cyan(k.padEnd(36))} ${pc.dim(val ?? "")}`);
          diffs++;
        } else if (k in local && !(k in remote)) {
          // Only in local
          const val = secret ? "********" : l;
          console.log(
            `  ${pc.yellow("local-only ")} ${pc.yellow(k.padEnd(36))} ${pc.dim(val ?? "")}`
          );
          diffs++;
        } else if (l !== r) {
          // Both exist but differ
          const lv = secret ? "********" : l;
          const rv = secret ? "********" : r;
          console.log(`  ${pc.red("changed    ")} ${pc.red(k.padEnd(36))}`);
          console.log(`    ${pc.dim("local :")} ${lv ?? pc.italic("(empty)")}`);
          console.log(`    ${pc.dim("remote:")} ${rv ?? pc.italic("(empty)")}`);
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
