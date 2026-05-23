import { test, expect, type Page } from '@playwright/test';

/**
 * E2E for PR 3.C.4 + 3.C.5 — URL state restoration + invalid-ws toast.
 *
 * On mount, `applyInitialRoute(sdk, route, …)` resolves `?ws=<id>`:
 *  - On a valid id, the workspace node is selected.
 *  - On an unknown id, a `toasts.warning(...)` fires and the URL clears.
 */

async function loginAndStay(page: Page, targetUrl: string) {
	await page.goto('/login');
	await page.getByLabel('Operator name').fill('E2E Operator');
	await page.getByLabel('Enterprise').check();
	await page.getByRole('button', { name: 'Begin' }).click();
	// Navigate directly to the target URL after auth lands.
	await page.goto(targetUrl);
	await page.waitForSelector(':root[data-hydrated]', { timeout: 10_000 });
}

test.describe('URL state restoration (PR 3.C)', () => {
	test('valid ?ws=<id> selects the workspace node', async ({ page }) => {
		// SDK calls workspaces.tree() during SSR-load; expose a node with id "ws-known".
		await page.route('**/v1/workspaces/tree', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify([
					{ id: 'ws-known', name: 'projects', kind: 'folder' },
				]),
			}),
		);
		// Client also calls workspaces.get(id) via applyInitialRoute.
		await page.route('**/v1/workspaces/ws-known', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify({ id: 'ws-known', name: 'projects', kind: 'folder' }),
			}),
		);

		await loginAndStay(page, '/?ws=ws-known');

		// The chat screen shows the breadcrumb / context chip for the selected node.
		await expect(page.getByText('projects').first()).toBeVisible();
	});

	test('invalid ?ws=<id> shows toast and clears URL', async ({ page }) => {
		await page.route('**/v1/workspaces/tree', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify([]),
			}),
		);
		await page.route('**/v1/workspaces/01HXNOTAREAL**', (route) =>
			route.fulfill({ status: 404, contentType: 'application/json', body: '{}' }),
		);

		await loginAndStay(page, '/?ws=01HXNOTAREAL000000000000000');

		// Toast appears within 1.5s.
		await expect(
			page.getByText('Workspace not found, returning to root'),
		).toBeVisible({ timeout: 1500 });
		// URL is cleared to root.
		await expect(page).toHaveURL('/');
	});
});
