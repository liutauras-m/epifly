/** GET /healthz — liveness probe. No auth, no upstream calls. */
import type { RequestHandler } from './$types.js';

export const GET: RequestHandler = () =>
	new Response('ok', { status: 200, headers: { 'content-type': 'text/plain', 'cache-control': 'no-store' } });
