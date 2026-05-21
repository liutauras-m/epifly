/**
 * Diagnostic — what happens when we fetch from inside the iOS Tauri WebView.
 */
import { browser, $, expect } from '@wdio/globals';
import * as crypto from 'crypto';

function makeSessionCookie(name: string, plan: string): string {
  const key = process.env.UI_SESSION_KEY ?? 'conusai-foundry-dev-secret-change-me-32b';
  const exp = Math.floor(Date.now() / 1000) + 3600;
  const payload = JSON.stringify({ name, plan, role: 'user', exp });
  const payloadB64 = Buffer.from(payload).toString('base64url');
  const mac = crypto.createHmac('sha256', key).update(payloadB64).digest('base64url');
  return `conusai_session=${payloadB64}.${mac}`;
}

async function switchToWebView() {
  await browser.waitUntil(
    async () => {
      const ctxs = await browser.getContexts();
      return ctxs.some((c) => (typeof c === 'string' ? c : c.id).includes('WEBVIEW'));
    },
    { timeout: 15_000 },
  );
  const ctxs = await browser.getContexts();
  const wv = ctxs.find((c) => (typeof c === 'string' ? c : c.id).includes('WEBVIEW'));
  await browser.switchContext(typeof wv === 'string' ? wv : wv!.id);
}

describe('Diag — fetch from inside WebView', () => {
  it('inspect window origin + cookie + fetch /v1/capabilities', async () => {
    await switchToWebView();

    // Set cookie via V15 pattern
    const c = makeSessionCookie('Diag', 'enterprise');
    const value = c.split('=')[1];
    await browser.execute(
      (v: string, n: string) => {
        document.cookie = `conusai_session=${v}; path=/; SameSite=Lax`;
        localStorage.setItem('conusai_shell_user', JSON.stringify({ name: n, plan: 'enterprise' }));
        localStorage.setItem('conusai_shell_token', v);
      },
      value,
      'Diag',
    );
    await browser.refresh();
    await browser.pause(3000);

    const probe = (await browser.execute(() => {
      return {
        origin: location.origin,
        href: location.href,
        cookie: document.cookie,
        hasToken: !!localStorage.getItem('conusai_shell_token'),
      };
    })) as any;
    // eslint-disable-next-line no-console
    console.log(`[DIAG] window = ${JSON.stringify(probe)}`);

    // Fire a fetch and inspect the result
    const key = `__diag_fetch_${Date.now()}`;
    await browser.execute(
      (k: string) => {
        (window as any)[k] = undefined;
        fetch('http://localhost:8080/v1/capabilities', { method: 'GET', credentials: 'include' })
          .then(async (res) => {
            const text = await res.text();
            (window as any)[k] = {
              ok: res.ok,
              status: res.status,
              ct: res.headers.get('content-type') ?? '',
              bodyHead: text.slice(0, 200),
              bodyLen: text.length,
            };
          })
          .catch((e: Error) => {
            (window as any)[k] = { error: e.message, errType: e.constructor.name };
          });
      },
      key,
    );

    await browser.pause(4000);
    const result = await browser.execute((k: string) => (window as any)[k], key);
    // eslint-disable-next-line no-console
    console.log(`[DIAG] fetch result = ${JSON.stringify(result)}`);

    // Also try X-Session-Token instead of cookie
    const key2 = `__diag_fetch2_${Date.now()}`;
    await browser.execute(
      (k: string, tok: string) => {
        (window as any)[k] = undefined;
        fetch('http://localhost:8080/v1/capabilities', {
          method: 'GET',
          headers: { 'X-Session-Token': tok },
        })
          .then(async (res) => {
            const text = await res.text();
            (window as any)[k] = {
              ok: res.ok,
              status: res.status,
              bodyHead: text.slice(0, 200),
            };
          })
          .catch((e: Error) => {
            (window as any)[k] = { error: e.message };
          });
      },
      key2,
      value,
    );
    await browser.pause(4000);
    const result2 = await browser.execute((k: string) => (window as any)[k], key2);
    // eslint-disable-next-line no-console
    console.log(`[DIAG] fetch w/ X-Session-Token = ${JSON.stringify(result2)}`);

    expect(true).toBe(true); // always pass — we just want logs
  });
});
