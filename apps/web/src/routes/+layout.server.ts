import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { firstName, initials } from '$lib/server/session';

export const load: LayoutServerLoad = ({ locals, url }) => {
	if (!locals.user && url.pathname !== '/login') redirect(302, '/login');
	if (!locals.user) return { user: null };
	const u = locals.user;
	return {
		user: {
			name: u.name,
			plan: u.plan.toUpperCase(),
			firstName: firstName(u.name),
			initials: initials(u.name)
		}
	};
};
