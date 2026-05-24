import { redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types.js';

export const load: PageServerLoad = async ({ locals, fetch }) => {
	if (!locals.user) redirect(302, '/login');

	// Load current subscription.
	let subscription = null;
	try {
		const res = await fetch('/v1/billing/subscription');
		if (res.ok) subscription = await res.json();
	} catch { /* billing not configured */ }

	return {
		user: locals.user,
		subscription,
		authProvider: process.env.CONUSAI_AUTH_PROVIDER ?? 'legacy',
	};
};
