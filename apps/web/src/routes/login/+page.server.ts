import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { COOKIE_NAME, sign, timeGreeting, verify } from '$lib/server/session';

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

		cookies.set(COOKIE_NAME, sign(name, plan), {
			path: '/',
			httpOnly: true,
			sameSite: 'lax',
			maxAge: 24 * 3600
		});

		redirect(302, '/');
	}
};
