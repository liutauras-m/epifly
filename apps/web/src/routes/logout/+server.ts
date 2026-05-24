import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { COOKIE_NAME } from '$lib/server/session.js';

export const GET: RequestHandler = ({ cookies }) => {
	cookies.delete(COOKIE_NAME, { path: '/' });
	redirect(302, '/login');
};
