/**
 * dokploy/lib/dokploy-client.mjs
 * Minimal Dokploy REST client using native fetch (Node 22+).
 * Zero dependencies — safe for use inside containers with no node_modules.
 *
 * Calls the Dokploy OpenAPI REST endpoints at ${baseUrl}/api/${procedure}:
 *   GET  procedures: params as URL query string (no tRPC envelope)
 *   POST procedures: params as plain JSON body (no tRPC envelope)
 *
 * Exposes the same makeClient(baseUrl, apiKey) interface as the old trpc.mjs
 * so callers need no changes beyond updating the import path.
 */

/**
 * Create a REST API client for Dokploy.
 *
 * @param {string} baseUrl  e.g. https://dokploy.example.com
 * @param {string} apiKey
 */
export function makeClient(baseUrl, apiKey) {
  const base = baseUrl.replace(/\/+$/, "");

  return {
    /**
     * Execute a REST GET procedure.
     * @param {string} procedure  e.g. "compose.search"
     * @param {object} input
     */
    async query(procedure, input) {
      const params = new URLSearchParams();
      for (const [k, v] of Object.entries(input ?? {})) {
        if (v !== undefined && v !== null) params.set(k, String(v));
      }
      const qs = params.toString();
      const url = `${base}/api/${procedure}${qs ? "?" + qs : ""}`;
      const res = await fetch(url, {
        headers: { "x-api-key": apiKey, accept: "application/json" },
      });
      return unwrap(res, procedure);
    },

    /**
     * Execute a REST POST procedure.
     * @param {string} procedure  e.g. "compose.deploy"
     * @param {object} input
     */
    async mutate(procedure, input) {
      const res = await fetch(`${base}/api/${procedure}`, {
        method: "POST",
        headers: {
          "x-api-key": apiKey,
          "content-type": "application/json",
          accept: "application/json",
        },
        body: JSON.stringify(input ?? {}),
      });
      return unwrap(res, procedure);
    },
  };
}

/**
 * Unwrap a REST response, throwing on HTTP errors.
 * @param {Response} res
 * @param {string} procedure  label for error messages
 * @returns {Promise<unknown>}
 */
async function unwrap(res, procedure) {
  const text = await res.text();
  /** @type {any} */
  let body = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    throw new Error(
      `${procedure}: non-JSON response (HTTP ${res.status}): ${text.slice(0, 200)}`,
    );
  }
  if (!res.ok) {
    const msg = body?.message ?? body?.error ?? text;
    throw new Error(`${procedure} → HTTP ${res.status}: ${msg}`);
  }
  return body;
}
