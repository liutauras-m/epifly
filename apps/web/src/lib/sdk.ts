import { createConusSdk } from '@conusai/sdk';
import { browser } from '$app/environment';

const cookieTokenProvider = {
  get: async () => null as string | null,
};

function resolveBackendUrl(): string {
	if (!browser) return 'http://localhost:8080';
	const origin = globalThis.location.origin;
	if (origin.includes(':3000')) return origin.replace(':3000', ':8080');
	if (origin.includes(':5173')) return origin.replace(':5173', ':8080');
	// Production: proxy API through the same origin (hooks.server.ts forwards to gateway)
	return origin;
}

export const sdk = createConusSdk({
	fetch: browser
	? ((input: RequestInfo | URL, init?: RequestInit) =>
		globalThis.fetch(input, { ...init, credentials: 'include' })) as typeof fetch
	: ((() => Promise.resolve(new Response())) as typeof fetch),
  baseUrl: resolveBackendUrl(),
  tokenProvider: cookieTokenProvider,
});
