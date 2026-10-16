import type { PageServerLoad } from './$types.js';
import { gatewayFetch, requireSession } from '$lib/server/gateway.js';

export const load: PageServerLoad = async ({ locals }) => {
	const session = requireSession(locals, '/account');

	let subscription = null;
	try {
		const res = await gatewayFetch(session)('/v1/billing/subscription');
		if (res.ok) subscription = await res.json();
	} catch {
		/* billing not configured */
	}

	return { user: session.user, subscription };
};
