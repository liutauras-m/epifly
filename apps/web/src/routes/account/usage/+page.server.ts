import type { PageServerLoad } from './$types.js';
import { gatewayFetch, requireSession } from '$lib/server/gateway.js';

export const load: PageServerLoad = async ({ locals }) => {
	const session = requireSession(locals, '/account/usage');
	const fetch = gatewayFetch(session);

	let usage = { agent_turns: 0, tokens: 0, storage_gb: 0 };
	let subscription = null;

	try {
		const [usageRes, subRes] = await Promise.allSettled([
			fetch('/v1/billing/usage'),
			fetch('/v1/billing/subscription'),
		]);
		if (usageRes.status === 'fulfilled' && usageRes.value.ok) {
			usage = await usageRes.value.json();
		}
		if (subRes.status === 'fulfilled' && subRes.value.ok) {
			subscription = await subRes.value.json();
		}
	} catch { /* billing not configured */ }

	return { user: session.user, usage, subscription };
};
