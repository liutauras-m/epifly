/**
 * dokploy/lib/compose-vars.mjs
 * Helpers for extracting ${VAR} references from compose YAML files and
 * rendering them as Dokploy ${{project.VAR}} per-compose env entries.
 * Zero dependencies.
 */

import { readFileSync } from "node:fs";

/**
 * Extract every `${VAR}`, `${VAR:-default}`, `${VAR:?msg}` etc. reference
 * from a compose YAML file. Returns a sorted deduplicated list.
 *
 * @param {string} composeFile  Absolute path to docker-compose.yml
 * @returns {string[]}
 */
export function extractComposeVars(composeFile) {
  const content = readFileSync(composeFile, "utf8");
  const re = /\$\{([A-Z_][A-Z0-9_]*)(?:[:?\-][^}]*)?\}/g;
  const vars = new Set();
  let m;
  while ((m = re.exec(content)) !== null) vars.add(m[1]);
  return [...vars].sort();
}

/**
 * Render a per-compose env block of pure Dokploy project-level references.
 * Dokploy expands `${{project.VAR}}` against Shared Env when writing each
 * service's .env file at deploy time; compose then interpolates as `${VAR}`.
 *
 * @param {string[]} vars
 * @returns {string}  Multi-line env string
 */
export function renderProjectRefs(vars) {
  return vars.map((v) => `${v}=\${{project.${v}}}`).join("\n") + "\n";
}
