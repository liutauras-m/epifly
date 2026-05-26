/**
 * tools/epifly/src/lib/dokploy.ts
 *
 * Typed async wrappers over the Dokploy REST API.
 * Uses native fetch (Node 22+) — no CLI binary, no spawnSync.
 *
 * GET  procedures → /api/{procedure}?key=val
 * POST procedures → /api/{procedure}  body: JSON
 */

export interface DokployConfig {
  dokployUrl: string;
  apiKey: string;
}

// ── HTTP helpers ──────────────────────────────────────────────────────────

async function get(
  cfg: DokployConfig,
  procedure: string,
  input?: Record<string, unknown>
): Promise<any> {
  const base = cfg.dokployUrl.replace(/\/+$/, "");
  const params = new URLSearchParams();
  for (const [k, v] of Object.entries(input ?? {})) {
    if (v !== undefined && v !== null) params.set(k, String(v));
  }
  const qs = params.toString();
  const url = `${base}/api/${procedure}${qs ? `?${qs}` : ""}`;
  const res = await fetch(url, {
    headers: { "x-api-key": cfg.apiKey, accept: "application/json" },
  });
  return unwrap(res, procedure);
}

async function post(
  cfg: DokployConfig,
  procedure: string,
  input?: Record<string, unknown>
): Promise<any> {
  const base = cfg.dokployUrl.replace(/\/+$/, "");
  const res = await fetch(`${base}/api/${procedure}`, {
    method: "POST",
    headers: {
      "x-api-key": cfg.apiKey,
      "content-type": "application/json",
      accept: "application/json",
    },
    body: JSON.stringify(input ?? {}),
  });
  return unwrap(res, procedure);
}

async function unwrap(res: Response, procedure: string): Promise<any> {
  const text = await res.text();
  let body: any = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    throw new Error(`${procedure}: non-JSON response (HTTP ${res.status}): ${text.slice(0, 200)}`);
  }
  if (!res.ok) {
    const msg = body?.message ?? body?.error ?? text.slice(0, 200);
    throw new Error(`${procedure} → HTTP ${res.status}: ${msg}`);
  }
  return body;
}

// ── Compose ───────────────────────────────────────────────────────────────

export async function searchComposes(
  cfg: DokployConfig,
  params: { environmentId: string; limit: number; offset: number }
): Promise<{ items: any[]; total: number }> {
  return get(cfg, "compose.search", params as Record<string, unknown>);
}

export async function getCompose(cfg: DokployConfig, composeId: string): Promise<any> {
  return get(cfg, "compose.one", { composeId });
}

export async function createCompose(
  cfg: DokployConfig,
  params: { name: string; environmentId: string; [key: string]: unknown }
): Promise<any> {
  return post(cfg, "compose.create", params as Record<string, unknown>);
}

export async function updateCompose(
  cfg: DokployConfig,
  params: { composeId: string; [key: string]: unknown }
): Promise<any> {
  return post(cfg, "compose.update", params as Record<string, unknown>);
}

export async function triggerDeploy(cfg: DokployConfig, composeId: string): Promise<any> {
  return post(cfg, "compose.deploy", { composeId });
}

export async function deleteCompose(
  cfg: DokployConfig,
  params: { composeId: string; deleteVolumes?: boolean }
): Promise<any> {
  return post(cfg, "compose.delete", params as Record<string, unknown>);
}

export async function readComposeLogs(
  cfg: DokployConfig,
  params: { composeId: string; containerId: string; tail?: number; since?: string; search?: string }
): Promise<any> {
  return get(cfg, "compose.readLogs", params as Record<string, unknown>);
}

// ── Deployment ────────────────────────────────────────────────────────────

export async function listDeploymentsByCompose(
  cfg: DokployConfig,
  composeId: string
): Promise<any[]> {
  return (await get(cfg, "deployment.allByCompose", { composeId })) ?? [];
}

export async function listDeploymentsByServer(
  cfg: DokployConfig,
  serverId: string
): Promise<any[]> {
  return (await get(cfg, "deployment.allByServer", { serverId })) ?? [];
}

export async function listDeploymentQueue(cfg: DokployConfig): Promise<any[]> {
  return (await get(cfg, "deployment.queueList")) ?? [];
}

// ── Server ────────────────────────────────────────────────────────────────

export async function listServers(cfg: DokployConfig): Promise<any[]> {
  return (await get(cfg, "server.all")) ?? [];
}

export async function getServerCount(cfg: DokployConfig): Promise<number> {
  const result = await get(cfg, "server.count");
  if (typeof result === "number") return result;
  if (typeof result?.count === "number") return result.count;
  return Number(result ?? 0) || 0;
}

// ── Docker ────────────────────────────────────────────────────────────────

export async function getServiceContainersByAppName(
  cfg: DokployConfig,
  params: { appName: string; serverId?: string }
): Promise<any[]> {
  return (
    (await get(cfg, "docker.getServiceContainersByAppName", params as Record<string, unknown>)) ??
    []
  );
}

export async function getContainersByAppNameMatch(
  cfg: DokployConfig,
  params: { appName: string; appType?: "stack" | "docker-compose"; serverId?: string }
): Promise<any[]> {
  return (
    (await get(cfg, "docker.getContainersByAppNameMatch", params as Record<string, unknown>)) ?? []
  );
}

// ── Project ───────────────────────────────────────────────────────────────

export async function getAllProjects(cfg: DokployConfig): Promise<any[]> {
  return (await get(cfg, "project.all")) ?? [];
}

export async function getProject(cfg: DokployConfig, projectId: string): Promise<any> {
  return get(cfg, "project.one", { projectId });
}

export async function updateProject(
  cfg: DokployConfig,
  params: { projectId: string; [key: string]: unknown }
): Promise<any> {
  return post(cfg, "project.update", params as Record<string, unknown>);
}

// ── Environment ───────────────────────────────────────────────────────────

export async function getEnvironment(cfg: DokployConfig, environmentId: string): Promise<any> {
  return get(cfg, "environment.one", { environmentId });
}

export async function listEnvironments(
  cfg: DokployConfig,
  params?: { projectId?: string; limit?: number; offset?: number }
): Promise<any[]> {
  const result = await get(
    cfg,
    "environment.search",
    params as Record<string, unknown> | undefined
  );
  return result?.items ?? result ?? [];
}

// ── Domain ────────────────────────────────────────────────────────────────

export async function getDomainsByCompose(cfg: DokployConfig, composeId: string): Promise<any[]> {
  return (await get(cfg, "domain.byComposeId", { composeId })) ?? [];
}

export async function createDomain(
  cfg: DokployConfig,
  params: Record<string, unknown>
): Promise<any> {
  return post(cfg, "domain.create", params);
}

export async function updateDomain(
  cfg: DokployConfig,
  params: { domainId: string; [key: string]: unknown }
): Promise<any> {
  return post(cfg, "domain.update", params as Record<string, unknown>);
}

export async function deleteDomain(cfg: DokployConfig, domainId: string): Promise<any> {
  return post(cfg, "domain.delete", { domainId });
}
