/**
 * epifly wipe — destructively wipe named Docker volumes on the Dokploy host.
 *
 *   epifly wipe --host user@server [--all] [--postgres] [--redis] ... [--yes]
 */

import type { Command } from "commander";
import { EXTERNAL_VOLUMES } from "../../../../dokploy/lib/manifest.mjs";
import { runOverSsh } from "../lib/ssh.ts";
import { loadConfig, readPartialConfig } from "../lib/config.ts";
import { banner, fatal, info, section, warn } from "../lib/ui.ts";
import { promptConfirm } from "../lib/prompts.ts";

const VOLUME_GROUPS: Record<string, string[]> = {
  postgres: ["conusai_postgres_data"],
  redis: ["conusai_redis_data"],
  qdrant: ["conusai_qdrant_data"],
  rustfs: ["conusai_rustfs_data", "conusai_redb_data"],
  lago: ["conusai_lago_storage_data"],
};

export function registerWipe(program: Command): void {
  program
    .command("wipe")
    .description("Destructively wipe named Docker volumes on the Dokploy host via SSH")
    .requiredOption("--host <user@host>", "SSH target for the Dokploy host")
    .option("--config <path>", "Path to .dokploy config file")
    .option("--all", "Wipe ALL managed volumes")
    .option("--postgres", "Wipe Postgres data volume")
    .option("--redis", "Wipe Redis data volume")
    .option("--qdrant", "Wipe Qdrant data volume")
    .option("--rustfs", "Wipe RustFS + Redb volumes")
    .option("--lago", "Wipe Lago storage volume")
    .option("--port <n>", "SSH port (default 22)")
    .option("--identity <file>", "SSH identity file")
    .option("--yes", "Skip confirmation prompt")
    .option("--no-backup", "Skip Postgres pg_dump before wiping")
    .action(async (opts) => {
      const volumes: string[] = [];

      if (opts.all) {
        volumes.push(...EXTERNAL_VOLUMES);
      } else {
        for (const [key, vols] of Object.entries(VOLUME_GROUPS)) {
          if ((opts as any)[key]) volumes.push(...vols);
        }
      }

      if (volumes.length === 0) {
        fatal(
          "No volumes selected.",
          "Use --all, --postgres, --redis, --qdrant, --rustfs, or --lago.",
        );
      }

      banner("wipe");
      section("⚠  DESTRUCTIVE OPERATION");
      warn("This will PERMANENTLY DELETE the following Docker volumes:");
      for (const v of volumes) warn(`  · ${v}`);
      console.log();

      if (!opts.yes) {
        const confirmed = await promptConfirm(
          "Type 'yes' to confirm you understand this will destroy all data in these volumes.",
          false,
        );
        if (!confirmed) {
          info("Wipe cancelled.");
          process.exit(0);
        }
      }

      // Build wipe-volumes.sh flags
      const flags: string[] = [];
      if (opts.all) flags.push("--all");
      if (opts.postgres) flags.push("--postgres");
      if (opts.redis) flags.push("--redis");
      if (opts.qdrant) flags.push("--qdrant");
      if (opts.rustfs) flags.push("--rustfs");
      if (opts.lago) flags.push("--lago");
      if (opts.noBackup) flags.push("--no-backup");
      flags.push("--yes"); // operator already confirmed locally

      const remoteScript = "/opt/epifly/scripts/wipe-volumes.sh";
      const cmd = `bash ${remoteScript} ${flags.join(" ")}`;

      info(`Connecting to ${opts.host}…`);
      info(`Running: ${cmd}`);
      console.log();

      try {
        runOverSsh(
          {
            host: opts.host,
            port: opts.port ? Number(opts.port) : undefined,
            identityFile: opts.identity,
          },
          cmd,
        );
      } catch (e: any) {
        fatal(`Wipe failed: ${e.message}`);
      }
    });
}
