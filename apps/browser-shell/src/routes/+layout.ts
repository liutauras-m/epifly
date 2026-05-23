// Tauri + SvelteKit requirement: the shell ships as a static SPA inside a
// webview, so SSR must be disabled and routes prerendered to plain HTML.
// See https://v2.tauri.app/start/frontend/sveltekit/
export const ssr = false;
export const prerender = true;
export const trailingSlash = 'always';
