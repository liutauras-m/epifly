import type { PageServerLoad } from './$types';
import { COOKIE_NAME } from '$lib/server/session';
import { createServerFetch } from '$lib/server/env';
import { apiCall } from '$lib/api/client';
import { glyphFor } from '$lib/api/glyphs';
import type { WorkspaceNode } from '$lib/types';
export type { WorkspaceNode };

export const load: PageServerLoad = async ({ locals, cookies }) => {
	if (!locals.user) return { recents: [], capabilities: [], workspaceTree: [] };

	const sessionCookie = cookies.get(COOKIE_NAME) ?? '';
	const serverFetch = createServerFetch(sessionCookie);

	type ThreadItem = { id: string; title?: string };
	type Cap = { name: string; kind?: string; tools?: unknown[] };

	const [threadsRes, capsRes, treeRes] = await Promise.allSettled([
		apiCall<ThreadItem[] | { data?: ThreadItem[]; items?: ThreadItem[] }>(
			serverFetch, '/v1/threads?limit=20'
		),
		apiCall<Cap[] | { capabilities?: Cap[]; data?: Cap[]; items?: Cap[] }>(
			serverFetch, '/v1/capabilities'
		),
		apiCall<WorkspaceNode[] | { nodes?: WorkspaceNode[] }>(
			serverFetch, '/v1/workspaces/tree'
		),
	]);

	const recents: { id: string; title: string }[] = (() => {
		if (threadsRes.status !== 'fulfilled' || threadsRes.value.error) return [];
		const raw = threadsRes.value.data;
		const arr: ThreadItem[] = Array.isArray(raw) ? raw : (raw?.data ?? raw?.items ?? []);
		return arr.map((t) => ({ id: t.id, title: t.title ?? 'Untitled thread' }));
	})();

	const capabilities: { name: string; kindGlyph: string; toolCount: number }[] = (() => {
		if (capsRes.status !== 'fulfilled' || capsRes.value.error) return [];
		const raw = capsRes.value.data;
		const arr: Cap[] = Array.isArray(raw) ? raw : (raw?.capabilities ?? raw?.data ?? raw?.items ?? []);
		return arr.map((c) => ({
			name: c.name,
			kindGlyph: glyphFor(c.kind ?? ''),
			toolCount: (c.tools?.length ?? 0)
		}));
	})();

	const workspaceTree: WorkspaceNode[] = (() => {
		if (treeRes.status !== 'fulfilled' || treeRes.value.error) return [];
		const raw = treeRes.value.data;
		return Array.isArray(raw) ? raw : (raw?.nodes ?? []);
	})();

	return { recents, capabilities, workspaceTree };
};
