/**
 * epifly status — show the deploy/health status of all compose services.
 *
 *   epifly status [--json]
 */

import type { Command } from "commander";
import { loadConfig } from "../lib/config.ts";
import { searchComposes } from "../lib/dokploy.ts";
import { banner, fatal, info, section, table } from "../lib/ui.ts";

const STATUS_COLOR: Record<string, string> = {
  done: "ok",
  running: "ok",
  error: "error",
  failed: "error",
  idle: "idle",
  queued: "queued",
};

export function registerStatus(program: Command): void {
  program
    .command("status")
    .description("Show deploy status of all compose services in the environment")
    .option("--config <path>", "Path to .dokploy config file")
    .option("--json", "Output as JSON")
    .action(async (opts) => {
      let cfg;
      try {
        cfg = loadConfig({ config: opts.config });
      } catch (e: any) {
        fatal(e.message);
      }

      let search: any;
      try {
        search = await searchComposes(cfg, {
          environmentId: cfg.environmentId,
          limit: 100,
          offset: 0,
        });
      } catch (e: any) {
        fatal(`Failed to fetch composes: ${e.message}`);
      }

      const items: any[] = search?.items ?? [];

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

      const rows: Array<[string, string, string]> = items.map((c) => [
        c.name ?? c.composeId,
        c.composeStatus ?? "unknown",
        STATUS_COLOR[c.composeStatus] ?? c.composeStatus,
      ]);

      table(rows);
    });
}
