/** Test stand-in for SvelteKit's `$env/dynamic/private`: the live process environment. */
export const env = process.env as Record<string, string | undefined>;
