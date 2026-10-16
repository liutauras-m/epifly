/**
 * Browser SDK. Every call goes to this app's own origin; the SvelteKit server
 * proxies it to the gateway and adds the bearer token from the encrypted
 * session cookie. No token is ever visible to browser code.
 */
import { createConusSdk } from '@conusai/sdk';
import { browser } from '$app/environment';

const browserOnly: typeof fetch = () => Promise.reject(new Error('the browser SDK is not available during SSR'));

export const sdk = createConusSdk({
	fetch: browser ? (input, init) => globalThis.fetch(input, init) : browserOnly,
	baseUrl: '',
	tokenProvider: { get: async () => null },
});
