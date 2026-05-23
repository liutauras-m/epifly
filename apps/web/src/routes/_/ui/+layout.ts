// Dev-only route guard — redirects to / in production builds.
// Uses SvelteKit's $app/environment `dev` flag (set by the build tool)
// rather than import.meta.env.DEV so SSR and CSR agree.
import { redirect } from '@sveltejs/kit';
import { dev } from '$app/environment';

export const load = () => {
  if (!dev) throw redirect(303, '/');
};
