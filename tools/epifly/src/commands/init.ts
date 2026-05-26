/**
 * epifly init — interactive bootstrap wizard.
 *
 * Guides the operator through:
 *   1. Collecting Dokploy URL, API key, environment ID, APP_DOMAIN
 *   2. Verifying connectivity to the Dokploy API
 *   3. Writing a .dokploy config file
 *   4. Optionally triggering a first deploy
 */

import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import type { Command } from "commander";
import * as p from "@clack/prompts";
import { banner, ok, section, warn } from "../lib/ui.ts";
import { listEnvironments } from "../lib/dokploy.ts";

export function registerInit(program: Command): void {
  program
    .command("init")
    .description("Interactive bootstrap wizard — create .dokploy config")
    .option("--config <path>", "Config file to write (default: .dokploy in cwd)")
    .action(async (opts) => {
      banner("init");

      p.intro("Let's configure your Epifly environment.");

      const dokployUrl = (await p.text({
        message: "Dokploy URL",
        placeholder: "https://dokploy.example.com",
        validate: (v) => {
          if (!v.startsWith("http")) return "Must start with http:// or https://";
        },
      })) as string;
      if (p.isCancel(dokployUrl)) { p.cancel("Cancelled."); process.exit(0); }

      const apiKey = (await p.password({
        message: "Dokploy API key",
        validate: (v) => (v.length < 8 ? "API key too short" : undefined),
      })) as string;
      if (p.isCancel(apiKey)) { p.cancel("Cancelled."); process.exit(0); }

      // Verify connectivity and list environments for the operator to choose.
      const sp = p.spinner();
      sp.start("Connecting to Dokploy API…");
      const initCfg = { dokployUrl: dokployUrl.replace(/\/+$/, ""), apiKey };

      let environmentId: string;
      try {
        const envList = await listEnvironments(initCfg);
        sp.stop("Connected");

        if (envList.length === 0) {
          warn("No environments found. Create one in the Dokploy UI first.");
          process.exit(1);
        }

        const chosen = (await p.select({
          message: "Select environment",
          options: envList.map((e: any) => ({
            value: e.environmentId,
            label: e.name ?? e.environmentId,
            hint: e.description ?? "",
          })),
        })) as string;
        if (p.isCancel(chosen)) { p.cancel("Cancelled."); process.exit(0); }
        environmentId = chosen;
      } catch (e: any) {
        sp.stop("Failed");
        p.log.error(`Cannot connect to Dokploy API: ${e.message}`);
        process.exit(1);
      }

      const appDomain = (await p.text({
        message: "APP_DOMAIN (e.g. epifly.prod.example.com)",
        placeholder: "epifly.prod.example.com",
        validate: (v) => {
          if (!v.includes(".")) return "Must be a valid domain";
        },
      })) as string;
      if (p.isCancel(appDomain)) { p.cancel("Cancelled."); process.exit(0); }

      const repoRoot = (await p.text({
        message: "Path to the conusai-platform repo root",
        defaultValue: process.cwd(),
        placeholder: process.cwd(),
      })) as string;
      if (p.isCancel(repoRoot)) { p.cancel("Cancelled."); process.exit(0); }

      const dest = opts.config ?? resolve(process.cwd(), ".dokploy");
      const cfg = {
        dokployUrl: dokployUrl.replace(/\/+$/, ""),
        apiKey,
        environmentId,
        appDomain,
        repoRoot,
      };
      writeFileSync(dest, JSON.stringify(cfg, null, 2) + "\n", "utf8");

      section("Done");
      ok(`Config written to ${dest}`);
      p.log.info("Run `epifly status` to check your environment, or `epifly deploy` to trigger a deploy.");
      p.outro("Happy deploying!");
    });
}
