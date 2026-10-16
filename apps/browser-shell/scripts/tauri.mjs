#!/usr/bin/env node
/**
 * Tauri CLI wrapper: pins the webview CSP `connect-src` (and `img-src`) to the
 * gateway origin of this build. tauri.conf.json carries a CSP with no remote
 * origin at all, so running the CLI without this wrapper fails closed.
 *
 * Build-time variables (public, baked into the bundle and the Rust binary):
 *   VITE_GATEWAY_URL               e.g. https://api.example.com
 *   VITE_ZITADEL_ISSUER            e.g. https://auth.example.com
 *   VITE_ZITADEL_NATIVE_CLIENT_ID  Zitadel native app client id
 *
 * The issuer is not in connect-src: only Rust talks to Zitadel.
 *
 * Usage: pnpm --filter browser-shell tauri build | dev | ios build | android build …
 */
import { spawnSync } from 'node:child_process';

const args = process.argv.slice(2);
const sub = ['ios', 'android'].includes(args[0]) ? args[1] : args[0];
const needsConfig = sub === 'dev' || sub === 'build';

if (needsConfig) {
	for (const name of ['VITE_GATEWAY_URL', 'VITE_ZITADEL_ISSUER', 'VITE_ZITADEL_NATIVE_CLIENT_ID']) {
		if (!process.env[name]) {
			console.error(`[tauri] ${name} is required for "${args.join(' ')}"`);
			process.exit(1);
		}
	}
	const gateway = new URL(process.env.VITE_GATEWAY_URL).origin;
	const devOrigins = sub === 'dev' ? ' http://localhost:5174 ws://localhost:5174' : '';
	const csp = [
		"default-src 'self'",
		`connect-src 'self' ipc: http://ipc.localhost ${gateway}${devOrigins}`,
		`img-src 'self' data: blob: ${gateway}`,
		"style-src 'self' 'unsafe-inline'",
		"script-src 'self'",
		"object-src 'none'",
		"base-uri 'self'",
		"frame-ancestors 'none'",
	].join('; ');
	args.push('--config', JSON.stringify({ app: { security: { csp } } }));
}

const res = spawnSync('tauri', args, { stdio: 'inherit', shell: process.platform === 'win32' });
process.exit(res.status ?? 1);
