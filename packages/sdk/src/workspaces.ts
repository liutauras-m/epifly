import type { WorkspaceNode } from '@conusai/types';
import type { InternalClient } from './client.js';
import type { ApiResult, WorkspaceContent, UploadResponse } from './types.js';
import { EP } from './endpoints.js';

export function workspaces(client: InternalClient) {
  return {
    tree(parentId?: string | null): Promise<ApiResult<WorkspaceNode[]>> {
      const url = parentId ? `${EP.WORKSPACES_TREE}?parent_id=${parentId}` : EP.WORKSPACES_TREE;
      return client.call('GET', url);
    },

    get(id: string): Promise<ApiResult<WorkspaceNode>> {
      return client.call('GET', EP.WORKSPACE_NODE(id));
    },

    create(body: { kind: string; name: string; parent_id?: string | null }): Promise<ApiResult<WorkspaceNode>> {
      return client.call('POST', EP.WORKSPACES, body);
    },

    search(q: string, limit = 20, mode?: 'semantic' | 'name'): Promise<ApiResult<WorkspaceNode[]>> {
      const params = new URLSearchParams({ q, limit: String(limit) });
      if (mode) params.set('mode', mode);
      return client.call('GET', `${EP.WORKSPACES_SEARCH}?${params}`);
    },

    getContent(id: string): Promise<ApiResult<WorkspaceContent>> {
      return client.call('GET', EP.WORKSPACE_CONTENT(id));
    },

    patchContent(id: string, content: string): Promise<ApiResult<WorkspaceNode>> {
      return client.call('PATCH', EP.WORKSPACE_CONTENT(id), { content });
    },

    move(id: string, body: { new_parent_id: string | null; new_parent_path: string | null }): Promise<ApiResult<WorkspaceNode>> {
      return client.call('POST', EP.WORKSPACE_MOVE(id), body);
    },

    rename(id: string, name: string): Promise<ApiResult<WorkspaceNode>> {
      return client.call('POST', EP.WORKSPACE_RENAME(id), { name });
    },

    delete(id: string): Promise<ApiResult<null>> {
      return client.call('DELETE', EP.WORKSPACE_NODE(id));
    },

    /**
     * Restore a paused thread projection.
     * Calls `POST /v1/threads/{threadId}/projection/restore`.
     * The `threadId` is the source thread ID (WorkspaceNode.source_id), not the node ID.
     */
    restoreThread(threadId: string): Promise<ApiResult<null>> {
      return client.call('POST', EP.THREAD_PROJECTION_RESTORE(threadId));
    },

    share(id: string, userId: string): Promise<ApiResult<WorkspaceNode>> {
      return client.call('POST', EP.WORKSPACE_SHARE(id), { user_id: userId });
    },

    unshare(id: string, userId: string): Promise<ApiResult<WorkspaceNode>> {
      return client.call('POST', EP.WORKSPACE_UNSHARE(id), { user_id: userId });
    },

    putTags(id: string, tags: string[]): Promise<ApiResult<WorkspaceNode>> {
      return client.call('PUT', EP.WORKSPACE_TAGS(id), { tags });
    },

    filterNodes(params: {
      tag?: string;
      kind?: 'folder' | 'file' | 'thread';
      since?: string;
      q?: string;
      limit?: number;
    }): Promise<ApiResult<WorkspaceNode[]>> {
      const qs = new URLSearchParams();
      if (params.tag) qs.set('tag', params.tag);
      if (params.kind) qs.set('kind', params.kind);
      if (params.since) qs.set('since', params.since);
      if (params.q) qs.set('q', params.q);
      if (params.limit != null) qs.set('limit', String(params.limit));
      const url = qs.toString() ? `${EP.WORKSPACES_FILTER}?${qs}` : EP.WORKSPACES_FILTER;
      return client.call('GET', url);
    },

    async upload(file: File): Promise<ApiResult<UploadResponse>> {
      const token = await client.tokenProvider.get();
      const fd = new FormData();
      fd.append('file', file, file.name);
      try {
        const res = await client.fetch(`${client.baseUrl}${EP.UI_UPLOAD}`, {
          method: 'POST',
          headers: token ? { Authorization: `Bearer ${token}` } : {},
          body: fd,
        });
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
    },
  };
}
