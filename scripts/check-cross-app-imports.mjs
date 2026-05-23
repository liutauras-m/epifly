#!/usr/bin/env node
// @ts-check
// Cross-app parity lint (PR 4.4).
//
// Enforces the §0.5 invariant from `docs/plan.md`:
//   - No file under `apps/web/src` may import a `.svelte` file from
//     `apps/browser-shell/...`, and vice versa.
//   - No `.svelte` file under `apps/<app>/src/lib/features/` is allowed,
//     unless it's on the narrow allow-list below. Feature components belong
//     in `packages/ui/src/lib/features/`.
//
// Run: `node scripts/check-cross-app-imports.mjs`
// Exit code: 0 clean, 1 violations.
//
// No dependencies — uses Node 20+ glob + fs.

import { readFile } from 'node:fs/promises';
import { glob } from 'node:fs/promises';
import { resolve, relative, basename } from 'node:path';

const ROOT = process.cwd();
const APPS = ['apps/web/src', 'apps/browser-shell/src'];

/**
 * Files that are allowed to live under `apps/*` even when their basename
 * suggests they look like a feature component. SvelteKit conventions (`+page`,
 * `+layout`, `+error`) plus app shell entry points.
 */
const ALLOWED_FEATURE_BASENAMES = new Set([
	'+page.svelte',
	'+layout.svelte',
	'+error.svelte',
	'MobileShell.svelte',
	'MobileTopBar.svelte',
]);

/**
 * Regexes for sniffing import specifiers from source text. Matches both
 *   import { x } from '...';
 *   import '...';
 * Dynamic `import('...')` is intentionally ignored.
 */
const STATIC_IMPORT_RE = /import\s+(?:[^;'"`]+?\s+from\s+)?['"]([^'"]+)['"]/g;

/** @typedef {{ file: string; spec: string; reason: string }} Violation */

/** @returns {Promise<string[]>} */
async function collectFiles() {
	const out = [];
	for (const dir of APPS) {
		const it = glob(`${dir}/**/*.{svelte,ts,js,svelte.ts}`, { cwd: ROOT, withFileTypes: false });
		for await (const path of it) {
			if (typeof path === 'string') out.push(resolve(ROOT, path));
		}
	}
	return out;
}

/** @param {string} file @returns {Promise<string[]>} */
async function importsOf(file) {
	let text = '';
	try { text = await readFile(file, 'utf8'); } catch { return []; }
	const out = [];
	for (const m of text.matchAll(STATIC_IMPORT_RE)) out.push(m[1]);
	return out;
}

/** @param {string} file */
function appOf(file) {
	const rel = relative(ROOT, file);
	if (rel.startsWith('apps/web/')) return 'web';
	if (rel.startsWith('apps/browser-shell/')) return 'browser-shell';
	return null;
}

async function main() {
	const files = await collectFiles();
	/** @type {Violation[]} */
	const violations = [];

	for (const file of files) {
		const rel = relative(ROOT, file);
		const ownApp = appOf(file);

		// Rule 2: feature components only allowed in packages/ui.
		if (
			rel.endsWith('.svelte') &&
			rel.includes('/src/lib/features/') &&
			!ALLOWED_FEATURE_BASENAMES.has(basename(rel)) &&
			!/\.test\.svelte$/.test(rel)
		) {
			violations.push({
				file: rel,
				spec: '',
				reason: 'feature .svelte components belong in packages/ui/src/lib/features/, not apps/*',
			});
		}

		const specs = await importsOf(file);
		for (const spec of specs) {
			// Rule 1: cross-app .svelte imports are forbidden.
			if (!spec.endsWith('.svelte')) continue;
			// Bare specifiers (e.g. '@conusai/ui/...') resolve outside the app and
			// are always allowed.
			if (!spec.startsWith('.') && !spec.startsWith('/')) continue;
			// Relative imports — check they don't cross app boundaries.
			const absoluteSpec = resolve(file, '..', spec);
			const targetApp = appOf(absoluteSpec);
			if (targetApp && ownApp && targetApp !== ownApp) {
				violations.push({
					file: rel,
					spec,
					reason: `cross-app import: ${ownApp} → ${targetApp}`,
				});
			}
		}
	}

	if (violations.length === 0) {
		console.log('cross-app parity lint: clean ✓');
		process.exit(0);
	}

	console.error('cross-app parity lint: violations\n');
	for (const v of violations) {
		console.error(`  ${v.file}`);
		if (v.spec) console.error(`    imports "${v.spec}"`);
		console.error(`    → ${v.reason}\n`);
	}
	console.error(`\n${violations.length} violation(s). See docs/plan.md §0.5 + §4.4 for the rule.`);
	process.exit(1);
}

main().catch((e) => {
	console.error(e);
	process.exit(2);
});
