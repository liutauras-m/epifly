/**
 * epifly secret — manage Shared Env secrets.
 *
 *   epifly secret list
 *   epifly secret rotate <KEY>
 *   epifly secret get <KEY>
 */

import type { Command } from "commander";
import pc from "picocolors";
import { isSecret, parseDotenv, renderDotenv } from "../../../../dokploy/lib/dotenv.mjs";
import { SECRETS, STATEFUL_SECRETS } from "../../../../dokploy/lib/secrets.mjs";
import { loadConfig } from "../lib/config.ts";
import { getEnvironment, getProject, updateProject } from "../lib/dokploy.ts";
import { promptConfirm } from "../lib/prompts.ts";
import { banner, err, fatal, info, ok, section, warn } from "../lib/ui.ts";

export function registerSecret(program: Command): void {
  const secret = program.command("secret").description("Manage Shared Env secrets");

  // ── epifly secret list ──────────────────────────────────────────────────────
  secret
    .command("list")
    .description("List all Shared Env keys (values masked for secrets)")
    .option("--config <path>", "Path to .dokploy config file")
    .option("--show", "Show secret values in plaintext")
    .action(async (opts) => {
      let cfg;
      try {
        cfg = loadConfig({ config: opts.config });
      } catch (e: any) {
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
        const masked =
          isSecret(k) && !opts.show ? pc.dim("********") : pc.green(v || pc.italic("(empty)"));
        const stateful = k in STATEFUL_SECRETS ? pc.yellow(" [stateful]") : "";
        const managed = k in SECRETS ? pc.blue(" [managed]") : "";
        console.log(`  ${k.padEnd(38)} ${masked}${stateful}${managed}`);
      }
    });

  // ── epifly secret get <KEY> ─────────────────────────────────────────────────
  secret
    .command("get <key>")
    .description("Print the current value of a single key")
    .option("--config <path>", "Path to .dokploy config file")
    .action(async (key: string, opts) => {
      let cfg;
      try {
        cfg = loadConfig({ config: opts.config });
      } catch (e: any) {
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
      // Print the raw value to stdout (suitable for `$(epifly secret get KEY)`)
      process.stdout.write(`${current[key]}\n`);
    });

  // ── epifly secret rotate <KEY> ──────────────────────────────────────────────
  secret
    .command("rotate <key>")
    .description("Regenerate a managed secret in Shared Env")
    .option("--config <path>", "Path to .dokploy config file")
    .option("--yes", "Skip confirmation prompt")
    .action(async (key: string, opts) => {
      let cfg;
      try {
        cfg = loadConfig({ config: opts.config });
      } catch (e: any) {
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
          `'${key}' is a STATEFUL secret bound to volume '${(STATEFUL_SECRETS as any)[key]}'. Rotating it will CORRUPT the data in that volume!`
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

      const newValue = (SECRETS as any)[key]();
      const merged = { ...current, [key]: newValue };
      const envText = renderDotenv(merged, projectRecord?.env ?? "");

      await updateProject(cfg, { projectId, env: envText });

      ok(`Rotated ${key}`);
      info(`New value: ${isSecret(key) ? "******** (masked)" : newValue}`);
      info("Run `epifly deploy` to apply the new secret to all services.");
    });
}
