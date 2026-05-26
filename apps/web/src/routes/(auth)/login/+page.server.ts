import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types.js';
import { COOKIE_NAME, sessionAdapter, timeGreeting, verify } from '$lib/server/session.js';

export const load: PageServerLoad = ({ cookies }) => {
	const raw = cookies.get(COOKIE_NAME);
	if (raw && verify(raw)) redirect(302, '/');
	return { greeting: timeGreeting() };
};

export const actions: Actions = {
	default: async ({ request, cookies }) => {
		const data = await request.formData();
		const name = (data.get('name') as string | null)?.trim() ?? '';
		const plan = (data.get('plan') as string | null) ?? 'enterprise';

		if (!name || name.length > 60) {
			return fail(400, { error: 'Name must be between 1 and 60 characters.', name });
		}

		const token = await sessionAdapter.issue(name, plan);

		cookies.set(COOKIE_NAME, token, {
			path: '/',
			httpOnly: true,
			sameSite: 'lax',
			maxAge: 24 * 3600
		});

		redirect(302, '/');
	}
};
