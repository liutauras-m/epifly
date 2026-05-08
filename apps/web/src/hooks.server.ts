import type { Handle } from '@sveltejs/kit';
import { COOKIE_NAME, verify } from '$lib/server/session';

export const handle: Handle = async ({ event, resolve }) => {
	const raw = event.cookies.get(COOKIE_NAME);
	event.locals.user = raw ? (verify(raw) ?? null) : null;
	return resolve(event);
};
