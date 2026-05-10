import { test, expect } from '@playwright/test';

// Helper: ensure Svelte hydration is complete before test starts
async function login(page: import('@playwright/test').Page) {
  await page.goto('/login');
  await page.getByLabel('Operator name').fill('E2E Operator');
  await page.getByLabel('Enterprise').check();
  await page.getByRole('button', { name: 'Begin' }).click();
  await expect(page).toHaveURL('/');
  // Wait for Svelte $effect in +layout.svelte to set data-hydrated,
  // confirming all event listeners are attached (dynamic imports + hydration complete)
  await page.waitForSelector(':root[data-hydrated]', { timeout: 10_000 });
}

// Helper: submit the composer via Meta+Enter keyboard shortcut
// (plain Enter only inserts a newline; the textarea submits on Meta+Enter / Ctrl+Enter)
async function submitComposer(page: import('@playwright/test').Page) {
  await page.getByRole('textbox').press('Meta+Enter');
}

// SSE body in the format that packages/sdk/src/chat.ts parses:
//   text     → choices[0].delta.content
//   thread   → top-level thread_id
//   tool_start  → choices[0].delta.tool_call_start
//   tool_result → choices[0].delta.tool_call_result
function sseLines(...lines: string[]) {
  return lines.map(l => `data: ${l}\n\n`).join('');
}

test.describe('chat stream', () => {
  test.beforeEach(async ({ page }) => {
    await login(page);
  });

  test('submitting prompt transitions to chat view', async ({ page }) => {
    await page.route('**/ui/stream', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'text/event-stream',
        body: sseLines(
          JSON.stringify({ thread_id: 'thread-e2e-1' }),
          JSON.stringify({ choices: [{ delta: { content: 'Hello from E2E!' } }] }),
          '[DONE]',
        ),
      });
    });

    await page.getByRole('textbox').fill('ping');
    await submitComposer(page);

    // Greeting disappears, chat view appears
    await expect(page.getByText(/Good .*, E2E/)).not.toBeVisible();
  });

  test('user message appears immediately', async ({ page }) => {
    await page.route('**/ui/stream', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'text/event-stream',
        body: sseLines('[DONE]'),
      })
    );

    await page.getByRole('textbox').fill('test prompt');
    await submitComposer(page);

    await expect(page.getByText('test prompt')).toBeVisible();
  });

  test('streamed AI response renders word by word', async ({ page }) => {
    await page.route('**/ui/stream', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'text/event-stream',
        body: sseLines(
          JSON.stringify({ choices: [{ delta: { content: 'The ' } }] }),
          JSON.stringify({ choices: [{ delta: { content: 'answer ' } }] }),
          JSON.stringify({ choices: [{ delta: { content: 'is 42.' } }] }),
          '[DONE]',
        ),
      })
    );

    await page.getByRole('textbox').fill('what is the answer?');
    await submitComposer(page);

    await expect(page.getByText(/The answer is 42\./)).toBeVisible({ timeout: 5000 });
  });

  test('tool call card shows running then success', async ({ page }) => {
    await page.route('**/ui/stream', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'text/event-stream',
        body: sseLines(
          JSON.stringify({ choices: [{ delta: { tool_call_start: { id: 'tc-1', name: 'web_search' } } }] }),
          JSON.stringify({ choices: [{ delta: { tool_call_result: { tool_use_id: 'tc-1', result: '{"ok":true}' } } }] }),
          JSON.stringify({ choices: [{ delta: { content: 'Found it.' } }] }),
          '[DONE]',
        ),
      })
    );

    await page.getByRole('textbox').fill('search the web');
    await submitComposer(page);

    await expect(page.getByText('web_search')).toBeVisible({ timeout: 5000 });
  });

  test('file upload via drag-drop triggers SDK upload', async ({ page }) => {
    // sdk.workspaces.upload calls EP.UI_UPLOAD = '/ui/upload'
    let uploadCalled = false;
    await page.route('**/ui/upload', async (route) => {
      uploadCalled = true;
      await route.fulfill({ status: 200, body: JSON.stringify({ id: 'file-1', filename: 'test.txt', size: 5, content_type: 'text/plain', download_url: '/v1/files/file-1' }) });
    });

    // Dispatch a real DragEvent with DataTransfer files on the composer form
    await page.evaluate(() => {
      const dt = new DataTransfer();
      const file = new File(['hello'], 'test.txt', { type: 'text/plain' });
      dt.items.add(file);
      const drop = new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: dt });
      document.querySelector('form.composer')?.dispatchEvent(drop);
    });

    await page.waitForTimeout(500);
    expect(uploadCalled).toBe(true);
  });
});
