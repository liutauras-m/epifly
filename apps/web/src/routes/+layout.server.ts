import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { firstName, initials } from '$lib/server/session';

export const load: LayoutServerLoad = ({ locals, url }) => {
	if (!locals.user && url.pathname !== '/login' && url.pathname !== '/_/ui') redirect(302, '/login');
	if (!locals.user) return { user: null };
	const u = locals.user;
	return {
		user: {
			name: u.name,
			plan: u.plan.toUpperCase(),
			firstName: firstName(u.name),
			initials: initials(u.name),
			/** Tenant identifier (PR 3.A.7) — used by client-side scope assert. */
			tenantId: u.tenantId ?? null
		}
	};
};
