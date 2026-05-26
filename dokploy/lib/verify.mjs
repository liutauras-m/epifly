/**
 * dokploy/lib/verify.mjs
 * HTTPS endpoint smoke-tests for Phase 5 / `epifly verify`.
 * Zero dependencies (uses native fetch, Node 22+).
 */

/**
 * Build the list of checks for a given APP_DOMAIN.
 *
 * @param {string} appDomain  e.g. "epifly.beta.test.cloud.conusai.com"
 * @returns {Array<{label: string, url: string, expectStatus: number[], expectJsonKey?: string}>}
 */
export function buildVerifyChecks(appDomain) {
  return [
    {
      label: "web (root)",
      url: `https://${appDomain}`,
      expectStatus: [200, 301, 302, 307, 308],
    },
    {
      label: "zitadel OIDC discovery",
      url: `https://auth.${appDomain}/.well-known/openid-configuration`,
      expectStatus: [200],
      expectJsonKey: "issuer",
    },
    {
      label: "lago health",
      url: `https://billing.${appDomain}/health`,
      expectStatus: [200, 301, 302],
    },
    {
      label: "rustfs S3 (anon)",
      url: `https://s3.${appDomain}/minio/health/live`,
      expectStatus: [200, 204, 403],
    },
    {
      label: "rustfs console",
      url: `https://s3-console.${appDomain}`,
      expectStatus: [200, 301, 302, 307, 308, 401, 403],
    },
    {
      label: "jaeger UI",
      url: `https://traces.${appDomain}`,
      expectStatus: [200, 301, 302, 307, 308],
    },
    {
      label: "gateway api health",
      url: `https://api.${appDomain}/health`,
      expectStatus: [200, 404], // 404 acceptable until route is added
    },
  ];
}

/**
 * Run a single HTTP check and return detailed diagnostics.
 * Timeout is 15 s.
 *
 * @param {{ url: string, expectStatus: number[], expectJsonKey?: string }} check
 * @returns {Promise<{
 *   ok: boolean,
 *   status?: number,
 *   error?: string,
 *   missingJsonKey?: string
 * }>}
 */
export async function runCheckDetailed({ url, expectStatus, expectJsonKey }) {
  const ac = new AbortController();
  const t = setTimeout(() => ac.abort(), 15_000);
  try {
    const res = await fetch(url, { redirect: "manual", signal: ac.signal });
    if (!expectStatus.includes(res.status)) {
      return {
        ok: false,
        status: res.status,
        error: `unexpected status ${res.status}, expected one of [${expectStatus.join(", ")}]`,
      };
    }
    if (expectJsonKey) {
      const body = await res.json().catch(() => null);
      if (!body || !(expectJsonKey in body)) {
        return {
          ok: false,
          status: res.status,
          error: "response JSON missing expected key",
          missingJsonKey: expectJsonKey,
        };
      }
    }
    return { ok: true, status: res.status };
  } catch (err) {
    if (err.name === "AbortError") {
      return { ok: false, error: `${url} timed out` };
    }
    return { ok: false, error: formatFetchError(err) };
  } finally {
    clearTimeout(t);
  }
}

/**
 * Produce actionable network/TLS errors for CLI output.
 *
 * @param {unknown} err
 * @returns {string}
 */
function formatFetchError(err) {
  const anyErr = /** @type {any} */ (err);
  const code = anyErr?.cause?.code || anyErr?.code;
  const msg = anyErr?.message || String(err);

  if (code === "UNABLE_TO_VERIFY_LEAF_SIGNATURE" || code === "CERT_HAS_EXPIRED") {
    return `TLS certificate verification failed (${code})`;
  }
  if (code === "DEPTH_ZERO_SELF_SIGNED_CERT" || code === "SELF_SIGNED_CERT_IN_CHAIN") {
    return `TLS certificate chain is not trusted (${code})`;
  }
  if (code === "ENOTFOUND") {
    return `DNS lookup failed (${code})`;
  }
  if (code === "ECONNREFUSED" || code === "ECONNRESET") {
    return `network connection failed (${code})`;
  }
  return code ? `${msg} (${code})` : msg;
}

/**
 * Backward-compatible boolean check API used by existing tests/callers.
 *
 * @param {{ url: string, expectStatus: number[], expectJsonKey?: string }} check
 * @returns {Promise<boolean>}
 */
export async function runCheck(check) {
  const result = await runCheckDetailed(check);
  return result.ok;
}
