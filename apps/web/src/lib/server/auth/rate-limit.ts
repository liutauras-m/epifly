/**
 * In-process sliding-window rate limiter for auth endpoints.
 *
 * Keyed by IP string prefix. Intentionally simple — no Redis dependency.
 * For high-scale deployments, replace with a shared backend (e.g. Upstash).
 *
 * Default: 10 requests per 60-second window per key.
 */

const WINDOW_MS = 60_000;
const DEFAULT_LIMIT = 10;

interface Bucket {
  count: number;
  windowStart: number;
}

const buckets = new Map<string, Bucket>();

/** Periodically purge stale buckets to prevent unbounded growth. */
let lastPurge = Date.now();
function maybePurge(): void {
  const now = Date.now();
  if (now - lastPurge < 300_000) return; // purge every 5 min
  lastPurge = now;
  for (const [key, b] of buckets) {
    if (now - b.windowStart >= WINDOW_MS * 2) buckets.delete(key);
  }
}

/**
 * Check whether the given key is within the rate limit.
 * Returns `true` if allowed, `false` if the limit is exceeded.
 */
export function checkRateLimit(key: string, limit = DEFAULT_LIMIT): boolean {
  maybePurge();
  const now = Date.now();
  const bucket = buckets.get(key);

  if (!bucket || now - bucket.windowStart >= WINDOW_MS) {
    buckets.set(key, { count: 1, windowStart: now });
    return true;
  }

  if (bucket.count >= limit) return false;
  bucket.count++;
  return true;
}

/**
 * Extract a rate-limit key from a SvelteKit request event.
 * Uses CF-Connecting-IP, X-Forwarded-For, or the direct socket address.
 */
export function rateLimitKey(headers: Headers): string {
  const cf = headers.get("cf-connecting-ip");
  if (cf) return cf.trim();

  const xff = headers.get("x-forwarded-for");
  if (xff) return xff.split(",")[0].trim();

  return "127.0.0.1";
}
