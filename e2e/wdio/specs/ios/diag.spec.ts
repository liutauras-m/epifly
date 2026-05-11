import { browser, $ } from '@wdio/globals';

describe('WebView diagnostic', () => {
  it('check native tree and contexts', async () => {
    const ctxs = await browser.getContexts();
    console.log('Contexts at launch:', JSON.stringify(ctxs, null, 2));

    await browser.pause(3000);
    const ctxs2 = await browser.getContexts();
    console.log('Contexts after 3s:', JSON.stringify(ctxs2, null, 2));

    const src = await browser.getPageSource();
    const hasWebView = src.includes('XCUIElementTypeWebView');
    console.log('Native tree has XCUIElementTypeWebView:', hasWebView);

    if (hasWebView) {
      const idx = src.indexOf('XCUIElementTypeWebView');
      console.log('WebView context around element:', src.substring(Math.max(0, idx - 50), idx + 300));
    }
  });
});
