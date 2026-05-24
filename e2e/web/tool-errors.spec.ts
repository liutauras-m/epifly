import { test, expect, type Page } from '@playwright/test';

/**
 * E2E for PR 3.D.3 — tool error toast.
 *
 * When the SSE `tool_call_result` delta carries `result` starting with
 * `"Error:"`, `createChatStream` marks the tool card failed and fires
 * `toasts.error(...)` with the gateway's verbatim message.
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

test.describe('tool error toast', () => {
	test('shows the gateway error verbatim on tool_call_result failure', async ({ page }) => {
		await page.route('**/ui/stream', (route) =>
			route.fulfill({
				status: 200,
				contentType: 'text/event-stream',
				body: sseLines(
					JSON.stringify({
						choices: [
							{
								delta: {
									tool_call_start: { id: 'tc-err', name: 'delete_node' },
								},
							},
						],
					}),
					JSON.stringify({
						choices: [
							{
								delta: {
									tool_call_result: {
										tool_use_id: 'tc-err',
										result: 'Error: node 01HFAKE0000000000000000000 not found',
									},
								},
							},
						],
					}),
					'[DONE]',
				),
			}),
		);

		await login(page);
		await page.getByRole('textbox').fill('delete node 01HFAKE0000000000000000000');
		await page.getByRole('textbox').press('Meta+Enter');

		// Toast appears within 2s with the gateway's message.
		await expect(
			page.locator('[data-testid="toast"]').getByText(/node 01HFAKE0000000000000000000 not found/),
		).toBeVisible({ timeout: 2000 });
	});
});
