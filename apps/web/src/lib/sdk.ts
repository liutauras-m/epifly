import { createConusSdk } from '@conusai/sdk';
import { browser } from '$app/environment';

const cookieTokenProvider = {
  get: async () => null as string | null,
};

export const sdk = createConusSdk({
  fetch: browser ? globalThis.fetch.bind(globalThis) : ((() => Promise.resolve(new Response())) as typeof fetch),
  baseUrl: '',
  tokenProvider: cookieTokenProvider,
});
