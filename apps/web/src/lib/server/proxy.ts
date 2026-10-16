/**
 * Same-origin reverse proxy to the gateway for `/v1`, `/api`, `/ui`, `/mcp`
 * and `/admin`.
 *
 * - The browser never holds a token: the proxy injects
 *   `Authorization: Bearer <access token>` from the encrypted session.
 * - Inbound `Authorization`, `Cookie`, `X-Tenant-ID`, `X-Session-Token`,
 *   API-key and hop-by-hop headers are dropped; `x-request-id` (generated when
 *   absent), `traceparent` and `tracestate` pass through.
 * - Non-safe methods must be same-origin (Origin == ORIGIN, or
 *   `Sec-Fetch-Site: same-origin` when a browser omits Origin).
 * - Request bodies stream through, capped at MAX_BODY_BYTES. Upstream must
 *   answer with headers within HEADERS_TIMEOUT_MS; buffered responses must
 *   finish within BODY_TIMEOUT_MS; event streams run until either side closes.
 * - WebSocket upgrades are not proxied (adapter-node has no upgrade hook).
 */
import type { RequestEvent } from '@sveltejs/kit';
import { config } from './config.js';

export const PROXIED_PREFIXES = ['/v1', '/api', '/ui', '/mcp', '/admin'] as const;

export const MAX_BODY_BYTES = 100 * 1024 * 1024;
const HEADERS_TIMEOUT_MS = 60_000;
const BODY_TIMEOUT_MS = 5 * 60_000;

const HOP_BY_HOP = new Set([
	'connection',
	'keep-alive',
	'proxy-authenticate',
	'proxy-authorization',
	'proxy-connection',
	'te',
	'trailer',
	'transfer-encoding',
	'upgrade',
]);

/** Request headers that must never reach the gateway from a browser. */
const DROPPED_REQUEST = new Set([
	'authorization',
	'cookie',
	'host',
	'origin',
	'referer',
	'content-length',
	'accept-encoding',
	'x-tenant-id',
	'x-session-token',
	'x-device-token',
	'x-api-key',
	'forwarded',
	'x-forwarded-for',
	'x-forwarded-host',
	'x-forwarded-proto',
	'x-real-ip',
]);

/** Response headers the gateway must not set on this origin. */
const DROPPED_RESPONSE = new Set(['set-cookie', 'content-encoding', 'content-length']);

const SAFE_METHODS = new Set(['GET', 'HEAD', 'OPTIONS']);

export function isProxiedPath(pathname: string): boolean {
	return PROXIED_PREFIXES.some((p) => pathname === p || pathname.startsWith(`${p}/`));
}

function jsonError(status: number, code: string): Response {
	return new Response(JSON.stringify({ error: code }), {
		status,
		headers: { 'content-type': 'application/json', 'cache-control': 'no-store' },
	});
}

export function isSameOrigin(request: Request, origin: string): boolean {
	const header = request.headers.get('origin');
	if (header !== null) return header === origin;
	return request.headers.get('sec-fetch-site') === 'same-origin';
}

/** Build the upstream request headers (exported for tests). */
export function upstreamHeaders(inbound: Headers, accessToken: string | null): Headers {
	const connectionListed = new Set(
		(inbound.get('connection') ?? '')
			.split(',')
			.map((h) => h.trim().toLowerCase())
			.filter(Boolean),
	);
	const out = new Headers();
	inbound.forEach((value, name) => {
		const lower = name.toLowerCase();
		if (HOP_BY_HOP.has(lower) || DROPPED_REQUEST.has(lower) || connectionListed.has(lower)) return;
		if (lower.startsWith('x-internal-')) return;
		out.set(lower, value);
	});
	if (accessToken) out.set('authorization', `Bearer ${accessToken}`);
	if (!out.has('x-request-id')) out.set('x-request-id', crypto.randomUUID());
	out.set('accept-encoding', 'identity');
	return out;
}

function downstreamHeaders(upstream: Headers): Headers {
	const out = new Headers();
	upstream.forEach((value, name) => {
		const lower = name.toLowerCase();
		if (HOP_BY_HOP.has(lower) || DROPPED_RESPONSE.has(lower) || lower.startsWith('x-internal-')) return;
		out.append(lower, value);
	});
	return out;
}

/** Cap a streaming request body at `limit` bytes. */
function limitBody(body: ReadableStream<Uint8Array>, limit: number): ReadableStream<Uint8Array> {
	let seen = 0;
	return body.pipeThrough(
		new TransformStream<Uint8Array, Uint8Array>({
			transform(chunk, controller) {
				seen += chunk.byteLength;
				if (seen > limit) controller.error(new Error('request body too large'));
				else controller.enqueue(chunk);
			},
		}),
	);
}

export async function proxyToGateway(event: RequestEvent): Promise<Response> {
	const { request, url, locals } = event;
	const cfg = config();

	if (!SAFE_METHODS.has(request.method) && !isSameOrigin(request, cfg.origin.origin)) {
		return jsonError(403, 'cross_origin_request_refused');
	}
	if (!locals.session) return jsonError(401, 'not_authenticated');

	const declared = Number(request.headers.get('content-length') ?? '0');
	if (declared > MAX_BODY_BYTES) return jsonError(413, 'payload_too_large');

	const target = new URL(`${url.pathname}${url.search}`, cfg.gatewayUrl);
	const headers = upstreamHeaders(request.headers, locals.session.accessToken);
	const hasBody = request.method !== 'GET' && request.method !== 'HEAD' && request.body !== null;

	const controller = new AbortController();
	const onClientAbort = () => controller.abort();
	request.signal.addEventListener('abort', onClientAbort, { once: true });
	const headersTimer = setTimeout(() => controller.abort(), HEADERS_TIMEOUT_MS);

	let upstream: Response;
	try {
		upstream = await fetch(target, {
			method: request.method,
			headers,
			body: hasBody ? limitBody(request.body!, MAX_BODY_BYTES) : undefined,
			signal: controller.signal,
			redirect: 'manual',
			// @ts-expect-error — undici needs duplex for a streaming request body
			duplex: hasBody ? 'half' : undefined,
		});
	} catch (e) {
		request.signal.removeEventListener('abort', onClientAbort);
		if (controller.signal.aborted) return jsonError(504, 'gateway_timeout');
		console.warn('[proxy] gateway unreachable:', e instanceof Error ? e.message : 'unknown');
		return jsonError(502, 'gateway_unavailable');
	} finally {
		clearTimeout(headersTimer);
	}

	const responseHeaders = downstreamHeaders(upstream.headers);
	const isEventStream = (upstream.headers.get('content-type') ?? '').startsWith('text/event-stream');
	if (isEventStream) {
		responseHeaders.set('cache-control', 'no-cache');
		responseHeaders.set('x-accel-buffering', 'no');
	}

	if (!upstream.body) {
		request.signal.removeEventListener('abort', onClientAbort);
		return new Response(null, { status: upstream.status, statusText: upstream.statusText, headers: responseHeaders });
	}

	const bodyTimer = isEventStream ? null : setTimeout(() => controller.abort(), BODY_TIMEOUT_MS);
	const done = () => {
		if (bodyTimer) clearTimeout(bodyTimer);
		request.signal.removeEventListener('abort', onClientAbort);
	};
	const body = upstream.body.pipeThrough(
		new TransformStream<Uint8Array, Uint8Array>({
			flush: done,
		}),
	);
	// Release timers if the client goes away mid-stream.
	controller.signal.addEventListener('abort', done, { once: true });

	return new Response(body, { status: upstream.status, statusText: upstream.statusText, headers: responseHeaders });
}
