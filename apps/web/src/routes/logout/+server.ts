import { redirect } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { COOKIE_NAME } from '$lib/server/session';

export const GET: RequestHandler = ({ cookies }) => {
	cookies.delete(COOKIE_NAME, { path: '/' });
	redirect(302, '/login');
};
