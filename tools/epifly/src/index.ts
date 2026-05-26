/**
 * Programmatic exports for tools/epifly.
 * Consumers (tests, other CLIs) can import commands and helpers directly.
 */

export { loadConfig, readPartialConfig, writeConfig } from "./lib/config.ts";
export type { EpiflyConfig, PartialConfig } from "./lib/config.ts";

export { banner, ok, warn, err, info, section, table, fatal, makeSpinner } from "./lib/ui.ts";
export { runOverSsh } from "./lib/ssh.ts";
export { tailDeployLogs } from "./lib/log-tail.ts";

export {
  promptText,
  promptPassword,
  promptConfirm,
  promptSelect,
  checkCancel,
  p,
} from "./lib/prompts.ts";
