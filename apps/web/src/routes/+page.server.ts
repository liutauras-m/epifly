import type { PageServerLoad } from './$types';
import { COOKIE_NAME } from '$lib/server/session';
import { createServerFetch, BACKEND_URL } from '$lib/server/env';
import { createConusSdk } from '@conusai/sdk';
import { glyphFor } from '@conusai/sdk';
import type { WorkspaceNode } from '@conusai/types';
export type { WorkspaceNode };

function makeServerSdk(sessionCookie: string) {
	const serverFetch = createServerFetch(sessionCookie);
	return createConusSdk({
		fetch: serverFetch,
		baseUrl: '',
		tokenProvider: { get: async () => null },
	});
}

export const load: PageServerLoad = async ({ locals, cookies }) => {
	if (!locals.user) return { recents: [], capabilities: [], workspaceTree: [] };

	const sessionCookie = cookies.get(COOKIE_NAME) ?? '';
	const sdk = makeServerSdk(sessionCookie);

	type ThreadItem = { id: string; title?: string };
	type Cap = { name: string; kind?: string; tools?: unknown[] };

	const [threadsRes, capsRes, treeRes] = await Promise.allSettled([
		sdk.threads.list({ limit: 20 }),
		sdk.capabilities.list(),
		sdk.workspaces.tree(),
	]);

	const recents: { id: string; title: string }[] = (() => {
		if (threadsRes.status !== 'fulfilled' || threadsRes.value.error) return [];
		const arr = threadsRes.value.data as ThreadItem[] | null;
		if (!Array.isArray(arr)) return [];
		return arr.map((t) => ({ id: t.id, title: t.title ?? 'Untitled thread' }));
	})();

	const capabilities: { name: string; kindGlyph: string; toolCount: number }[] = (() => {
		if (capsRes.status !== 'fulfilled' || capsRes.value.error) return [];
		const raw = capsRes.value.data as { capabilities?: Cap[] } | Cap[] | null;
		const arr: Cap[] = Array.isArray(raw) ? raw : (raw?.capabilities ?? []);
		return arr.map((c) => ({
			name: c.name,
			kindGlyph: glyphFor(c.kind ?? ''),
			toolCount: (c.tools?.length ?? 0),
		}));
	})();

	const workspaceTree: WorkspaceNode[] = (() => {
		if (treeRes.status !== 'fulfilled' || treeRes.value.error) return [];
		const raw = treeRes.value.data as WorkspaceNode[] | { nodes?: WorkspaceNode[] } | null;
		return Array.isArray(raw) ? raw : (raw?.nodes ?? []);
	})();

	return { recents, capabilities, workspaceTree };
};
