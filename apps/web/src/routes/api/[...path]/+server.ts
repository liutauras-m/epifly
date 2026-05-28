/**
 * Reverse proxy: /api/[...path] → backend gateway.
 *
 * Security model:
 * - Header allowlist (forward only what the backend legitimately needs)
 * - Injects Authorization: Bearer from server-side session (never from client)
 * - Strips x-tenant-id and all x-internal-* headers
 * - Path is normalized and constrained to /v1/** and /healthz
 * - Method allowlist: GET POST PUT PATCH DELETE
 * - Body size limit: 25 MiB (non-SSE), streaming for SSE
 * - Timeout: 60s default, 5 min for SSE; client disconnect cancels upstream
 */
import { error } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import { env } from "$env/dynamic/private";

const BACKEND_URL = env.BACKEND_URL ?? "http://localhost:8080";
const MAX_BODY_BYTES = 25 * 1024 * 1024; // 25 MiB
const TIMEOUT_DEFAULT_MS = 60_000;
const TIMEOUT_SSE_MS = 5 * 60_000;

// Upstream prefixes that are allowed
const ALLOWED_PREFIXES = ["/v1/", "/healthz"];

// HTTP methods the proxy accepts
const ALLOWED_METHODS = new Set(["GET", "POST", "PUT", "PATCH", "DELETE"]);

// Headers we forward from the client (allowlist)
const FORWARDED_CLIENT_HEADERS = new Set([
  "accept",
  "accept-language",
  "content-type",
  "cache-control",
  "x-request-id",
]);

// Headers we strip unconditionally even if present in upstream response
const STRIPPED_RESPONSE_HEADERS = new Set([
  "transfer-encoding",
  "connection",
  "keep-alive",
]);

function normalizePath(raw: string): string | null {
  // Remove leading slashes duplicates, reject traversal
  const decoded = decodeURIComponent(raw);
  if (decoded.includes("..") || decoded.includes("//")) return null;
  const normalized = "/" + decoded.replace(/^\/+/, "");
  const allowed = ALLOWED_PREFIXES.some(
    (p) => normalized === p.slice(0, -1) || normalized.startsWith(p)
  );
  return allowed ? normalized : null;
}

export const GET = handler;
export const POST = handler;
export const PUT = handler;
export const PATCH = handler;
export const DELETE = handler;

async function handler({ params, request, locals }: Parameters<RequestHandler>[0]) {
  if (!ALLOWED_METHODS.has(request.method)) throw error(405, "method_not_allowed");

  const upstreamPath = normalizePath(params.path);
  if (!upstreamPath) throw error(400, "path_not_allowed");

  // Session must be present (all /api routes are authenticated)
  const session = locals.session;
  if (!session) throw error(401, "not_authenticated");

  const isSSE =
    request.headers.get("accept")?.includes("text/event-stream") === true;

  // Build upstream request headers (allowlist)
  const upstreamHeaders = new Headers();
  upstreamHeaders.set("Authorization", `Bearer ${session.accessToken}`);

  for (const name of FORWARDED_CLIENT_HEADERS) {
    const val = request.headers.get(name);
    if (val) upstreamHeaders.set(name, val);
  }

  // Generate a request id if none was provided
  if (!upstreamHeaders.has("x-request-id")) {
    upstreamHeaders.set("x-request-id", crypto.randomUUID());
  }

  // Enforce body size limit for non-SSE requests
  let body: BodyInit | null = null;
  if (request.method !== "GET" && request.method !== "HEAD") {
    if (isSSE) {
      body = request.body;
    } else {
      const contentLength = parseInt(request.headers.get("content-length") ?? "0", 10);
      if (contentLength > MAX_BODY_BYTES) throw error(413, "payload_too_large");
      const buf = await request.arrayBuffer();
      if (buf.byteLength > MAX_BODY_BYTES) throw error(413, "payload_too_large");
      body = buf;
    }
  }

  const upstreamUrl = `${BACKEND_URL}${upstreamPath}`;

  const timeoutMs = isSSE ? TIMEOUT_SSE_MS : TIMEOUT_DEFAULT_MS;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  // Propagate client disconnect to upstream
  request.signal.addEventListener("abort", () => controller.abort());

  let upstreamRes: Response;
  try {
    upstreamRes = await fetch(upstreamUrl, {
      method: request.method,
      headers: upstreamHeaders,
      body,
      // @ts-expect-error — Node fetch supports duplex streaming
      duplex: isSSE ? "half" : undefined,
      signal: controller.signal,
    });
  } catch (e) {
    clearTimeout(timeout);
    if (controller.signal.aborted) throw error(504, "upstream_timeout");
    console.error("[proxy] upstream fetch error:", e instanceof Error ? e.message : e);
    throw error(502, "upstream_unavailable");
  } finally {
    if (!isSSE) clearTimeout(timeout);
  }

  // Build response headers (strip hop-by-hop and sensitive headers)
  const responseHeaders = new Headers();
  upstreamRes.headers.forEach((val, name) => {
    const lower = name.toLowerCase();
    if (STRIPPED_RESPONSE_HEADERS.has(lower)) return;
    if (lower.startsWith("x-internal-")) return;
    responseHeaders.set(name, val);
  });

  if (isSSE) {
    // Forward SSE stream; clear timeout when stream ends
    const stream = new ReadableStream({
      async start(ctrl) {
        if (!upstreamRes.body) { ctrl.close(); clearTimeout(timeout); return; }
        const reader = upstreamRes.body.getReader();
        try {
          while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            ctrl.enqueue(value);
          }
        } catch {
          // client disconnected or upstream closed
        } finally {
          ctrl.close();
          clearTimeout(timeout);
        }
      },
    });
    return new Response(stream, {
      status: upstreamRes.status,
      headers: responseHeaders,
    });
  }

  return new Response(upstreamRes.body, {
    status: upstreamRes.status,
    headers: responseHeaders,
  });
}
