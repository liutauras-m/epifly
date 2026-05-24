import { test, expect, type Page } from '@playwright/test';

/**
 * E2E for PR 3.A.9 — live workspace + recents + optimistic rollback.
 *
 * These tests mock the SDK HTTP endpoints rather than spinning up a real
 * gateway. The contract under test is the *client* behaviour:
 * - `resource_invalidated` SSE deltas trigger a tree re-fetch (PR 3.A).
 * - `createLiveResource.mutate({ rollbackOn })` reverts + toasts on rejection
 *   (PR 3.A.4.1).
 * - The recents list refreshes on `resource: "threads"` invalidations (PR 3.A.6).
 */

async function login(page: Page) {
	await page.goto('/login');
	await page.getByLabel('Operator name').fill('E2E Operator');
	await page.getByLabel('Enterprise').check();
	await page.getByRole('button', { name: 'Begin' }).click();
	await expect(page).toHaveURL('/');
	await page.waitForSelector(':root[data-hydrated]', { timeout: 10_000 });
}

function sseLines(...lines: string[]): string {
	return lines.map((l) => `data: ${l}\n\n`).join('');
}

test.describe('live resources — workspace & recents', () => {
	test('chat-driven workspace mutation refreshes sidebar', async ({ page }) => {
		page.on('console', msg => console.log('  [BROWSER] ->', msg.text()));
		// Mock the initial tree (empty) and a second tree that includes the new node.
		let treeCallCount = 0;
		await page.route('**/v1/workspaces/tree', async (route) => {
			treeCallCount++;
			const body =
				treeCallCount === 1
					? []
					: [{ id: 'wsnode-1', name: 'notes', kind: 'folder' }];
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(body),
			});
		});

		await login(page);

		// Drive a single chat turn whose SSE stream includes a resource_invalidated
		// delta — that should trigger a re-fetch of /v1/workspaces/tree.
		await page.route('**/ui/stream', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'text/event-stream',
				body: sseLines(
					JSON.stringify({
						choices: [
							{
								delta: {
									resource_invalidated: {
										resource: 'workspace',
										scope: 'tenant-e2e',
										changed_keys: ['notes'],
									},
								},
							},
						],
					}),
					'[DONE]',
				),
			}),
		);

		await page.getByRole('textbox').fill('save notes/test.md');
		await page.getByRole('textbox').press('Meta+Enter');

		// Wait for stream to complete (composer becomes enabled again)
		await expect(page.getByRole('textbox')).toBeEnabled();

		// Sidebar shows the new folder within ~1.5s after the SSE delta.
		await expect(page.locator('.node-name', { hasText: 'notes' })).toBeVisible({ timeout: 1500 });
		expect(treeCallCount).toBeGreaterThanOrEqual(2);
	});

	test('recents list updates on threads invalidation', async ({ page }) => {
		// Initial threads list empty; subsequent list returns a thread.
		let threadCallCount = 0;
		await page.route('**/v1/threads?**', async (route) => {
			threadCallCount++;
			const body =
				threadCallCount === 1
					? { data: [] }
					: { data: [{ id: 'tid-1', title: 'My new chat' }] };
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(body),
			});
		});
		await page.route('**/v1/threads', async (route) => {
			// Match the no-query variant if SDK omits ?limit=
			threadCallCount++;
			const body =
				threadCallCount === 1
					? { data: [] }
					: { data: [{ id: 'tid-1', title: 'My new chat' }] };
			await route.fulfill({
				status: 200,
				contentType: 'application/json',
				body: JSON.stringify(body),
			});
		});

		await login(page);

		// Drive a chat turn whose SSE stream emits resource_invalidated for threads.
		await page.route('**/ui/stream', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'text/event-stream',
				body: sseLines(
					JSON.stringify({
						choices: [
							{
								delta: {
									resource_invalidated: {
										resource: 'threads',
										scope: 'tenant-e2e',
										changed_keys: ['tid-1'],
									},
								},
							},
						],
					}),
					'[DONE]',
				),
			}),
		);

		await page.getByRole('textbox').fill('hello');
		await page.getByRole('textbox').press('Meta+Enter');

		// Recents row appears within 1.5s.
		await expect(page.getByText('My new chat')).toBeVisible({ timeout: 1500 });
	});
});
