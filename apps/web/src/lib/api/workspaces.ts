import { apiCall } from './client';
import { EP } from './endpoints';
import type { ApiResult, WorkspaceContent, UploadResponse } from './types';
import type { WorkspaceNode } from '$lib/types';

export function getTree(
  fetchFn: typeof fetch,
  parentId?: string | null
): Promise<ApiResult<WorkspaceNode[] | { nodes: WorkspaceNode[] }>> {
  const url = parentId ? `${EP.WORKSPACES_TREE}?parent_id=${parentId}` : EP.WORKSPACES_TREE;
  return apiCall(fetchFn, url);
}

export function getNode(fetchFn: typeof fetch, id: string): Promise<ApiResult<WorkspaceNode>> {
  return apiCall(fetchFn, EP.WORKSPACE_NODE(id));
}

export function createNode(
  fetchFn: typeof fetch,
  body: { kind: string; name: string; parent_id?: string | null }
): Promise<ApiResult<WorkspaceNode>> {
  return apiCall(fetchFn, EP.WORKSPACES, { method: 'POST', body: JSON.stringify(body) });
}

export function searchNodes(
  fetchFn: typeof fetch,
  q: string,
  limit = 20
): Promise<ApiResult<WorkspaceNode[] | { nodes: WorkspaceNode[] }>> {
  return apiCall(fetchFn, `${EP.WORKSPACES_SEARCH}?q=${encodeURIComponent(q)}&limit=${limit}`);
}

export function getContent(
  fetchFn: typeof fetch,
  id: string
): Promise<ApiResult<WorkspaceContent>> {
  return apiCall(fetchFn, EP.WORKSPACE_CONTENT(id));
}

export function patchContent(
  fetchFn: typeof fetch,
  id: string,
  content: string
): Promise<ApiResult<WorkspaceNode>> {
  return apiCall(fetchFn, EP.WORKSPACE_CONTENT(id), {
    method: 'PATCH',
    body: JSON.stringify({ content })
  });
}

export function moveNode(
  fetchFn: typeof fetch,
  id: string,
  body: { new_parent_id: string | null; new_parent_path: string | null }
): Promise<ApiResult<WorkspaceNode>> {
  return apiCall(fetchFn, EP.WORKSPACE_MOVE(id), { method: 'POST', body: JSON.stringify(body) });
}

export function deleteNode(fetchFn: typeof fetch, id: string): Promise<ApiResult<null>> {
  return apiCall(fetchFn, EP.WORKSPACE_NODE(id), { method: 'DELETE' });
}

export function shareNode(fetchFn: typeof fetch, id: string, userId: string): Promise<ApiResult<WorkspaceNode>> {
  return apiCall(fetchFn, EP.WORKSPACE_SHARE(id), { method: 'POST', body: JSON.stringify({ user_id: userId }) });
}

export function unshareNode(fetchFn: typeof fetch, id: string, userId: string): Promise<ApiResult<WorkspaceNode>> {
  return apiCall(fetchFn, EP.WORKSPACE_UNSHARE(id), { method: 'POST', body: JSON.stringify({ user_id: userId }) });
}

/**
 * Upload a single file as multipart/form-data.
 * Cannot go through apiCall because apiCall forces Content-Type: application/json.
 * This is the single authorised raw-fetch site for uploads (lives in the API layer).
 */
export async function uploadFile(fetchFn: typeof fetch, file: File): Promise<ApiResult<UploadResponse>> {
  const fd = new FormData();
  fd.append('file', file, file.name);
  try {
    const res = await fetchFn(EP.UI_UPLOAD, { method: 'POST', body: fd });
    if (!res.ok) {
      let message = `HTTP ${res.status}`;
      try { const j = await res.json(); message = (j as { error?: string }).error ?? message; } catch {}
      return { data: null, error: { status: res.status, message } };
    }
    const data = await res.json() as UploadResponse;
    return { data, error: null };
  } catch (e: unknown) {
    return { data: null, error: { status: 0, message: e instanceof Error ? e.message : String(e) } };
  }
}
