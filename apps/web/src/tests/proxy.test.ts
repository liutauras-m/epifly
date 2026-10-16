import { describe, expect, it } from 'vitest';
import { isProxiedPath, isSameOrigin, upstreamHeaders } from '$lib/server/proxy.js';

describe('isProxiedPath', () => {
	it.each([
		['/v1/threads', true],
		['/v1', true],
		['/api/realtime', true],
		['/ui/stream', true],
		['/mcp', true],
		['/admin/capabilities', true],
		['/_/ui', false],
		['/uikit', false],
		['/account', false],
		['/healthz', false],
	])('%s → %s', (path, expected) => {
		expect(isProxiedPath(path)).toBe(expected);
	});
});

describe('upstreamHeaders', () => {
	const inbound = new Headers({
		authorization: 'Bearer forged',
		cookie: '__Host-epifly_session=abc',
		'x-tenant-id': 'other-tenant',
		'x-session-token': 'legacy',
		'x-api-key': 'k',
		connection: 'keep-alive, x-custom-hop',
		'x-custom-hop': '1',
		'transfer-encoding': 'chunked',
		'content-type': 'application/json',
		accept: 'text/event-stream',
		traceparent: '00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01',
		'x-request-id': 'req-1',
		'x-internal-secret': 's',
	});

	it('replaces the credential with the session bearer and drops tenant/session headers', () => {
		const out = upstreamHeaders(inbound, 'real-token');
		expect(out.get('authorization')).toBe('Bearer real-token');
		for (const h of ['cookie', 'x-tenant-id', 'x-session-token', 'x-api-key', 'x-custom-hop', 'transfer-encoding', 'connection', 'x-internal-secret']) {
			expect(out.has(h), h).toBe(false);
		}
	});

	it('keeps tracing and content headers', () => {
		const out = upstreamHeaders(inbound, 'real-token');
		expect(out.get('traceparent')).toBe(inbound.get('traceparent'));
		expect(out.get('x-request-id')).toBe('req-1');
		expect(out.get('content-type')).toBe('application/json');
		expect(out.get('accept')).toBe('text/event-stream');
	});

	it('generates a request id when absent and sends no bearer without a session', () => {
		const out = upstreamHeaders(new Headers(), null);
		expect(out.get('x-request-id')).toMatch(/^[0-9a-f-]{36}$/);
		expect(out.has('authorization')).toBe(false);
	});
});

describe('isSameOrigin', () => {
	const origin = 'https://app.example.test';
	const req = (headers: Record<string, string>) => new Request('https://app.example.test/v1/x', { method: 'POST', headers });

	it('accepts a matching Origin', () => expect(isSameOrigin(req({ origin }), origin)).toBe(true));
	it('refuses a foreign Origin', () => expect(isSameOrigin(req({ origin: 'https://evil.example' }), origin)).toBe(false));
	it('falls back to Sec-Fetch-Site', () => {
		expect(isSameOrigin(req({ 'sec-fetch-site': 'same-origin' }), origin)).toBe(true);
		expect(isSameOrigin(req({ 'sec-fetch-site': 'cross-site' }), origin)).toBe(false);
		expect(isSameOrigin(req({}), origin)).toBe(false);
	});
});
