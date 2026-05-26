/**
 * Config loader for the epifly CLI.
 * Merges: .dokploy (JSON config file) → env vars → CLI flags.
 *
 * Config file location precedence:
 *   1. --config flag
 *   2. $EPIFLY_CONFIG env var
 *   3. ./.dokploy
 *   4. ~/.dokploy
 */

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";

export interface EpiflyConfig {
  /** Dokploy base URL, e.g. https://dokploy.example.com */
  dokployUrl: string;
  /** Dokploy API key */
  apiKey: string;
  /** Dokploy environment ID */
  environmentId: string;
  /** APP_DOMAIN, e.g. epifly.prod.cloud.conusai.com */
  appDomain: string;
  /** Path to the repo root (for reading compose files, domains.yaml, etc.) */
  repoRoot: string;
}

export interface PartialConfig extends Partial<EpiflyConfig> {
  /** Source that provided this config (file path or "env") */
  _source?: string;
}

const CONFIG_SEARCH = [
  resolve(process.cwd(), ".dokploy"),
  resolve(process.cwd(), ".epifly.json"),
  resolve(homedir(), ".dokploy"),
];

function readConfigFile(path: string): PartialConfig {
  if (!existsSync(path)) return {};
  const raw = readFileSync(path, "utf8");
  try {
    return { ...JSON.parse(raw), _source: path };
  } catch {
    // Support shell-style .dokploy files, e.g. KEY="value" lines.
    const out: PartialConfig = { _source: path };
    for (const line of raw.split(/\r?\n/)) {
      const t = line.trim();
      if (!t || t.startsWith("#")) continue;
      const m = t.match(/^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/);
      if (!m) continue;
      let value = m[2].trim();
      if (
        (value.startsWith('"') && value.endsWith('"')) ||
        (value.startsWith("'") && value.endsWith("'"))
      ) {
        value = value.slice(1, -1);
      }

      if (m[1] === "DOKPLOY_URL") out.dokployUrl = value;
      if (m[1] === "DOKPLOY_API_KEY") out.apiKey = value;
      if (m[1] === "DOKPLOY_ENVIRONMENT_ID") out.environmentId = value;
      if (m[1] === "APP_DOMAIN") out.appDomain = value;
    }

    if (out.dokployUrl || out.apiKey || out.environmentId || out.appDomain) {
      return out;
    }

    throw new Error(`Failed to parse config file ${path}: invalid JSON or env format`);
  }
}

/** Read config from file + env, without requiring all fields to be present. */
export function readPartialConfig(
  flags: {
    config?: string;
    dokployUrl?: string;
    apiKey?: string;
    environmentId?: string;
    appDomain?: string;
    repoRoot?: string;
  } = {}
): PartialConfig {
  const explicitConfigPath = flags.config ? resolve(process.cwd(), flags.config) : undefined;

  // 1. Config file
  const configPath =
    explicitConfigPath || process.env.EPIFLY_CONFIG || CONFIG_SEARCH.find((p) => existsSync(p));
  const file = configPath ? readConfigFile(configPath) : {};

  // 2. Env vars
  const env: PartialConfig = {
    ...(process.env.DOKPLOY_URL ? { dokployUrl: process.env.DOKPLOY_URL } : {}),
    ...(process.env.DOKPLOY_API_KEY ? { apiKey: process.env.DOKPLOY_API_KEY } : {}),
    ...(process.env.DOKPLOY_ENVIRONMENT_ID
      ? { environmentId: process.env.DOKPLOY_ENVIRONMENT_ID }
      : {}),
    ...(process.env.APP_DOMAIN ? { appDomain: process.env.APP_DOMAIN } : {}),
  };

  // 3. Merge: CLI flags override env override file
  const merged: PartialConfig = {
    repoRoot: process.cwd(),
    ...file,
    ...env,
    ...(flags.dokployUrl ? { dokployUrl: flags.dokployUrl } : {}),
    ...(flags.apiKey ? { apiKey: flags.apiKey } : {}),
    ...(flags.environmentId ? { environmentId: flags.environmentId } : {}),
    ...(flags.appDomain ? { appDomain: flags.appDomain } : {}),
    ...(flags.repoRoot ? { repoRoot: flags.repoRoot } : {}),
  };
  return merged;
}

/** Load and validate full config, throwing a user-friendly error if required keys are missing. */
export function loadConfig(flags: Parameters<typeof readPartialConfig>[0] = {}): EpiflyConfig {
  const cfg = readPartialConfig(flags);
  const required: Array<keyof EpiflyConfig> = [
    "dokployUrl",
    "apiKey",
    "environmentId",
    "appDomain",
  ];
  const missing = required.filter((k) => !cfg[k]);
  if (missing.length > 0) {
    throw new Error(
      `Missing required config: ${missing.join(", ")}\n\nSet them via:\n  • epifly init            (interactive wizard)\n  • .dokploy config file   (JSON)\n  • Environment variables  (DOKPLOY_URL, DOKPLOY_API_KEY, DOKPLOY_ENVIRONMENT_ID, APP_DOMAIN)`
    );
  }
  return {
    dokployUrl: cfg.dokployUrl!.replace(/\/+$/, ""),
    apiKey: cfg.apiKey!,
    environmentId: cfg.environmentId!,
    appDomain: cfg.appDomain!,
    repoRoot: cfg.repoRoot ?? process.cwd(),
  };
}

/** Write config to .dokploy in cwd. */
export function writeConfig(
  cfg: Partial<EpiflyConfig>,
  dest = resolve(process.cwd(), ".dokploy")
): void {
  const existing = existsSync(dest) ? JSON.parse(readFileSync(dest, "utf8")) : {};
  writeFileSync(dest, `${JSON.stringify({ ...existing, ...cfg }, null, 2)}\n`, "utf8");
}
