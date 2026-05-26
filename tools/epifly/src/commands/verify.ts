/**
 * epifly verify — run HTTPS smoke-tests against a live environment.
 *
 *   epifly verify [--domain <app-domain>]
 */

import type { Command } from "commander";
import { buildVerifyChecks, runCheckDetailed } from "../../../../dokploy/lib/verify.mjs";
import { readPartialConfig } from "../lib/config.ts";
import { banner, err, fatal, info, ok, section, warn } from "../lib/ui.ts";

export function registerVerify(program: Command): void {
  program
    .command("verify")
    .description("Run HTTPS smoke-tests against a live environment")
    .option("-d, --domain <domain>", "APP_DOMAIN to verify (overrides config)")
    .option("--config <path>", "Path to .dokploy config file")
    .action(async (opts) => {
      const partial = readPartialConfig({ config: opts.config, appDomain: opts.domain });
      const appDomain = partial.appDomain;
      if (!appDomain)
        fatal("No APP_DOMAIN. Pass --domain or set it in .dokploy / APP_DOMAIN env var.");

      banner("verify");
      section(`Checking ${appDomain}`);

      const checks = buildVerifyChecks(appDomain);
      const failures: typeof checks = [];

      for (const check of checks) {
        const result = await runCheckDetailed(check);
        const label = check.label.padEnd(52);
        if (result.ok) {
          const status = result.status !== undefined ? ` [${result.status}]` : "";
          ok(`${label} ${check.url}${status}`);
        } else {
          const status = result.status !== undefined ? ` [${result.status}]` : "";
          const reason = result.error ?? "check failed";
          const keyHint = result.missingJsonKey
            ? ` (missing JSON key: ${result.missingJsonKey})`
            : "";
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
