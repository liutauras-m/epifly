import { COOKIE_NAME } from './session';

export const BACKEND_URL = process.env.CONUSAI_BACKEND_URL ?? 'http://localhost:8080';

/**
 * Create a fetch wrapper that prepends BACKEND_URL and injects the session
 * cookie for server-side load functions. Pass the result directly to apiCall.
 */
export function createServerFetch(sessionCookie: string): typeof fetch {
	const authHeaders: Record<string, string> = {
		Cookie: `${COOKIE_NAME}=${sessionCookie}`,
		'Content-Type': 'application/json'
	};
	return ((url: string, init?: RequestInit) =>
		fetch(`${BACKEND_URL}${url}`, {
			...init,
			headers: { ...authHeaders, ...(init?.headers ?? {}) }
		})) as unknown as typeof fetch;
}
