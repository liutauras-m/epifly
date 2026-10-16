/**
 * Shell SDK: calls the public gateway (VITE_GATEWAY_URL) directly with
 * `Authorization: Bearer <access token>`. The token comes from Rust on each
 * request (Rust refreshes it when needed) and is never persisted in the webview.
 */
import { createConusSdk } from '@conusai/sdk';
import { invoke } from '@tauri-apps/api/core';
import { sessionExpired } from './session.svelte.js';

const GATEWAY_URL = (import.meta.env.VITE_GATEWAY_URL as string | undefined)?.replace(/\/+$/, '');
if (!GATEWAY_URL) throw new Error('VITE_GATEWAY_URL must be set at build time');

async function gatewayFetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
	const url = typeof input === 'string' && input.startsWith('/') ? `${GATEWAY_URL}${input}` : input;
	let token: string;
	try {
		token = await invoke<string>('auth_access_token');
	} catch (e) {
		sessionExpired(typeof e === 'string' ? e : undefined);
		throw e instanceof Error ? e : new Error(String(e));
	}
	const headers = new Headers(init?.headers);
	headers.set('Authorization', `Bearer ${token}`);
	const res = await globalThis.fetch(url, { ...init, headers, credentials: 'omit' });
	if (res.status === 401) sessionExpired();
	return res;
}

export const sdk = createConusSdk({
	fetch: gatewayFetch as typeof fetch,
	baseUrl: '',
	// The fetch wrapper adds the bearer to every call (including uploads and
	// streams that bypass the token provider).
	tokenProvider: { get: async () => null },
});

// E2E hook — exposes the SDK to Appium test code (real WebKit fetch). Guarded
// behind a build-time flag so production bundles never include it.
if (typeof window !== 'undefined' && import.meta.env.VITE_E2E_EXPOSE_SDK) {
	(window as { __conusaiSdk?: typeof sdk }).__conusaiSdk = sdk;
}
