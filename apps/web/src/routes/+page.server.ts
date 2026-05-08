import type { PageServerLoad } from './$types';
import { COOKIE_NAME } from '$lib/server/session';

const BACKEND = process.env.CONUSAI_BACKEND_URL ?? 'http://localhost:8080';

export const load: PageServerLoad = async ({ locals, cookies }) => {
	if (!locals.user) return { recents: [], capabilities: [] };

	const sessionCookie = cookies.get(COOKIE_NAME) ?? '';
	const headers: Record<string, string> = {
		Cookie: `${COOKIE_NAME}=${sessionCookie}`,
		'Content-Type': 'application/json'
	};

	const [threadsRes, capsRes] = await Promise.allSettled([
		fetch(`${BACKEND}/v1/threads?limit=20`, { headers }),
		fetch(`${BACKEND}/v1/capabilities`, { headers })
	]);

	const recents: { id: string; title: string }[] = await (async () => {
		if (threadsRes.status !== 'fulfilled' || !threadsRes.value.ok) return [];
		try {
			const data = await threadsRes.value.json();
			const arr = Array.isArray(data) ? data : (data?.data ?? data?.items ?? []);
			return (arr as { id: string; title?: string }[]).map((t) => ({
				id: t.id,
				title: t.title ?? 'Untitled thread'
			}));
		} catch { return []; }
	})();

	type Cap = { name: string; kind?: string; tools?: unknown[] };
	const capabilities: { name: string; kindGlyph: string; toolCount: number }[] = await (async () => {
		if (capsRes.status !== 'fulfilled' || !capsRes.value.ok) return [];
		try {
			const data = await capsRes.value.json();
			const arr = Array.isArray(data) ? data : (data?.data ?? data?.items ?? []);
			return (arr as Cap[]).map((c) => ({
				name: c.name,
				kindGlyph: glyphFor(c.kind ?? ''),
				toolCount: c.tools?.length ?? 0
			}));
		} catch { return []; }
	})();

	return { recents, capabilities };
};

function glyphFor(kind: string): string {
	const k = kind.toLowerCase();
	if (k.includes('mcp')) return 'M';
	if (k.includes('wasm')) return 'W';
	if (k.includes('docker')) return 'D';
	if (k.includes('pipeline') || k.includes('chain')) return 'P';
	if (k.includes('native') || k.includes('builtin')) return 'N';
	return '·';
}
