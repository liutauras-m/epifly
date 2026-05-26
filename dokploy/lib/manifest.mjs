/**
 * dokploy/lib/manifest.mjs
 * Declarative project manifest — apps, volumes, derived env.
 * Zero dependencies. Shared by deploy.mjs (orchestrator) and tools/epifly (CLI).
 */

/** @type {Array<{name: string, composePath: string, hasDomains: boolean}>} */
export const APPS = [
  { name: "infra",         composePath: "./dokploy/infra/docker-compose.yml",         hasDomains: true },
  { name: "gateway",       composePath: "./dokploy/gateway/docker-compose.yml",       hasDomains: true },
  { name: "web",           composePath: "./dokploy/web/docker-compose.yml",           hasDomains: true },
  { name: "observability", composePath: "./dokploy/observability/docker-compose.yml", hasDomains: true },
  { name: "capabilities",  composePath: "./dokploy/capabilities/docker-compose.yml",  hasDomains: false },
];

/**
 * External named Docker volumes. Declared `external: true` in compose files
 * so lifecycle is decoupled from the compose project. Created by Phase 0.
 */
export const EXTERNAL_VOLUMES = [
  "conusai_postgres_data",
  "conusai_redis_data",
  "conusai_qdrant_data",
  "conusai_rustfs_data",
  "conusai_redb_data",
  "conusai_lago_storage_data",
];

/**
 * Env vars that are derived from APP_DOMAIN rather than stored independently.
 * Recomputed every deploy so changing APP_DOMAIN propagates everywhere.
 *
 * @param {string} appDomain
 * @returns {Record<string, string>}
 */
export function DERIVED(appDomain) {
  return {
    ZITADEL_ISSUER: `https://auth.${appDomain}`,
    COOKIE_DOMAIN: `.${appDomain}`,
  };
}

export const DOCKER_SOCK = "/var/run/docker.sock";
