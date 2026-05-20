/**
 * One-shot invoice extraction test — uses running simulator, attaches to
 * existing app session, switches to WebView, sends extraction prompt.
 */
import { browser, $ } from '@wdio/globals';
import * as fs from 'fs';

const INVOICE_URL = 'http://host.docker.internal:9090/invoice.png';

async function switchToWebView() {
  await browser.waitUntil(
    async () => {
      const ctxs = await browser.getContexts();
      return ctxs.some((c) => (typeof c === 'string' ? c : c.id).includes('WEBVIEW'));
    },
    { timeout: 20_000, timeoutMsg: 'No WEBVIEW context' },
  );
  const ctxs = await browser.getContexts();
  const wv = ctxs.find((c) => (typeof c === 'string' ? c : c.id).includes('WEBVIEW'));
  await browser.switchContext(typeof wv === 'string' ? wv : wv!.id);
}

describe('Invoice extraction test', () => {
  it('sends invoice URL and gets extraction response', async () => {
    const ctxs = await browser.getContexts();
    const ctxIds = ctxs.map((c) => (typeof c === 'string' ? c : c.id));
    console.log('Contexts:', JSON.stringify(ctxIds));

    if (ctxIds.some((id) => id.includes('WEBVIEW'))) {
      await switchToWebView();
    }

    // Type the invoice extraction message
    const textarea = await $('textarea, [role="textbox"]');
    await textarea.waitForExist({ timeout: 10_000 });
    await textarea.setValue(`Extract the invoice at ${INVOICE_URL} and return the invoice number, status, and total amount`);
    await browser.pause(500);

    // Send via button or Enter
    const sendBtn = await $('button[aria-label*="Send"], button[type="submit"]:not([form])');
    if (await sendBtn.isExisting()) {
      await sendBtn.click();
    } else {
      await browser.execute(() => {
        const ta = document.querySelector('textarea');
        if (!ta) return;
        ta.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', metaKey: true, bubbles: true }));
      });
    }

    console.log('Message sent, waiting for AI response...');

    // Wait up to 90s for the response to contain key invoice data
    await browser.waitUntil(
      async () => {
        const body = await browser.execute(() => document.body.innerText) as string;
        return body.includes('HCY') || body.includes('PAID') || body.includes('63.99') || body.includes('63,99');
      },
      { timeout: 90_000, timeoutMsg: 'Invoice data not found in response' },
    );

    const responseText = await browser.execute(() => document.body.innerText) as string;
    console.log('Response (excerpt):', responseText.slice(-1000));

    // Take screenshot
    const screenshot = await browser.takeScreenshot();
    fs.writeFileSync('/tmp/sim_invoice_result.png', Buffer.from(screenshot, 'base64'));
    console.log('Screenshot saved to /tmp/sim_invoice_result.png');
  });
});
