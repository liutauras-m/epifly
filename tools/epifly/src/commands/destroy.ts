/**
 * epifly destroy — delete all compose services in the configured environment.
 *
 *   epifly destroy [--yes] [--delete-volumes]
 */

import type { Command } from "commander";
import { loadConfig } from "../lib/config.ts";
import { searchComposes, deleteCompose, deleteDomain, getDomainsByCompose } from "../lib/dokploy.ts";
import { banner, fatal, info, ok, section, warn } from "../lib/ui.ts";
import { promptConfirm } from "../lib/prompts.ts";

export function registerDestroy(program: Command): void {
  program
    .command("destroy")
    .description("Delete all compose services in the environment (irreversible)")
    .option("--config <path>", "Path to .dokploy config file")
    .option("--yes", "Skip confirmation prompt")
    .option("--delete-volumes", "Also delete Docker volumes inside each compose")
    .action(async (opts) => {
      let cfg: any;
      try {
        cfg = loadConfig({ config: opts.config });
      } catch (e: any) {
        fatal(e.message);
      }

      banner("destroy");
      section("⚠  DESTRUCTIVE OPERATION");

      // List all composes
      let items: any[];
      try {
        const search = await searchComposes(cfg, {
          environmentId: cfg.environmentId,
          limit: 100,
          offset: 0,
        });
        items = search?.items ?? [];
      } catch (e: any) {
        fatal(`Failed to list composes: ${e.message}`);
      }

      if (items!.length === 0) {
        info("No compose services found in this environment.");
        return;
      }

      warn(`This will permanently delete ${items!.length} compose service(s):`);
      for (const c of items!) {
        warn(`  · ${c.name} (${c.composeId}) — status: ${c.composeStatus}`);
      }
      console.log();

      if (!opts.yes) {
        const confirmed = await promptConfirm(
          "Are you sure you want to delete all these services?",
          false,
        );
        if (!confirmed) {
          info("Destroy cancelled.");
          process.exit(0);
        }
      }

      section("Deleting composes");

      let failed = 0;
      for (const c of items!) {
        // Delete domains first to avoid orphaned traefik rules
        try {
          const domains = await getDomainsByCompose(cfg, c.composeId);
          for (const d of domains ?? []) {
            try {
              await deleteDomain(cfg, d.domainId);
            } catch {}
          }
        } catch {}

        try {
          await deleteCompose(cfg, {
            composeId: c.composeId,
            deleteVolumes: Boolean(opts.deleteVolumes),
          });
          ok(`  ✓ ${c.name}`);
        } catch (e: any) {
          const message = String(e?.message ?? e);
          if (!opts.deleteVolumes && message.includes("--deleteVolumes")) {
            warn(`  ✗ ${c.name}: Dokploy CLI requires --deleteVolumes on this server version.`);
            warn("    Re-run with --delete-volumes or upgrade Dokploy CLI/server for optional flag behavior.");
          } else {
            warn(`  ✗ ${c.name}: ${message || "delete failed"}`);
          }
          failed++;
        }
      }

      console.log();
      if (failed > 0) {
        fatal(`${failed}/${items!.length} deletes failed. Check the Dokploy UI.`);
      } else {
        ok(`All ${items!.length} services deleted. Run \`epifly deploy\` to redeploy.`);
      }
    });
}
