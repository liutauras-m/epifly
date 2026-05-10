import type { APIRequestContext } from '@playwright/test';

const BASE_URL = process.env.E2E_API_URL ?? 'http://localhost:8080';

/**
 * Creates a test workspace node via the API.
 * Requires a valid JWT in E2E_JWT env or falls back to test-mode (no auth).
 */
export async function seedWorkspaceNode(
  request: APIRequestContext,
  opts: { name: string; kind?: string } = { name: 'e2e-test-node' }
): Promise<string | null> {
  const token = process.env.E2E_JWT;
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  try {
    const res = await request.post(`${BASE_URL}/v1/workspaces`, {
      headers,
      data: { name: opts.name, kind: opts.kind ?? 'folder' },
    });
    if (!res.ok()) return null;
    const body = await res.json();
    return body.id ?? null;
  } catch {
    return null;
  }
}

export async function deleteWorkspaceNode(
  request: APIRequestContext,
  nodeId: string
): Promise<void> {
  const token = process.env.E2E_JWT;
  const headers: Record<string, string> = {};
  if (token) headers['Authorization'] = `Bearer ${token}`;
  try {
    await request.delete(`${BASE_URL}/v1/workspaces/${nodeId}`, { headers });
  } catch {}
}
