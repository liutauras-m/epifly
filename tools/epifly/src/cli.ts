/**
 * epifly CLI entrypoint.
 * Registers all subcommands and delegates to commander.
 */

import { Command } from "commander";
import { registerDeploy } from "./commands/deploy.ts";
import { registerDestroy } from "./commands/destroy.ts";
import { registerDiff } from "./commands/diff.ts";
import { registerDoctor } from "./commands/doctor.ts";
import { registerInit } from "./commands/init.ts";
import { registerLogs } from "./commands/logs.ts";
import { registerSecret } from "./commands/secret.ts";
import { registerStatus } from "./commands/status.ts";
import { registerVerify } from "./commands/verify.ts";
import { registerWipe } from "./commands/wipe.ts";

const program = new Command("epifly")
  .version("0.1.0")
  .description("Operator CLI for Epifly — manage Dokploy-hosted stacks");

registerInit(program);
registerDeploy(program);
registerDestroy(program);
registerLogs(program);
registerVerify(program);
registerSecret(program);
registerStatus(program);
registerDiff(program);
registerWipe(program);
registerDoctor(program);

program.parseAsync(process.argv).catch((err) => {
  console.error(err instanceof Error ? err.message : err);
  process.exit(1);
});
