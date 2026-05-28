import type { Handle } from "@sveltejs/kit";
import { sequence } from "@sveltejs/kit/hooks";
import { loadSession, refreshSession } from "$lib/server/auth/session";
import { getOidcConfig } from "$lib/server/auth/oidc";
import { auditRefreshFailure } from "$lib/server/auth/audit";
import { env } from "$env/dynamic/private";
import * as client from "openid-client";

const SESSION_COOKIE = "__Host-epifly_sid";
/** Refresh the access token when it expires within this many ms. */
const REFRESH_THRESHOLD_MS = 60_000;

// ── Session handler ────────────────────────────────────────────────────────────

const session: Handle = async ({ event, resolve }) => {
  const sid = event.cookies.get(SESSION_COOKIE);

  if (sid) {
    try {
      const loaded = await loadSession(sid);

      if (loaded) {
        let accessToken = loaded.accessToken;

        try {
          const expMs = getJwtExpMs(accessToken);
          if (expMs !== null && expMs - Date.now() < REFRESH_THRESHOLD_MS) {
            const cfg = await getOidcConfig();
            const rotated = await refreshSession({
              sessionId: sid,
              refreshFn: async (oldRefreshToken) => {
                const tokenSet = await client.refreshTokenGrant(
                  cfg.serverConfig,
                  oldRefreshToken
                );
                const exp = tokenSet.expires_in ?? 3600;
                return {
                  accessToken: tokenSet.access_token,
                  refreshToken: tokenSet.refresh_token ?? oldRefreshToken,
                  accessExpiresAt: new Date(Date.now() + exp * 1000),
                };
              },
            });

            if (rotated === null) {
              auditRefreshFailure({ reason: "invalid_grant" });
              event.cookies.delete(SESSION_COOKIE, { path: "/" });
            } else {
              accessToken = rotated;
            }
          }
        } catch (e) {
          auditRefreshFailure({ reason: e instanceof Error ? e.message : "unknown" });
          // Non-fatal: skip proactive refresh; the proxy handles 401s
        }

        event.locals.session = {
          userIss: loaded.userIss,
          userSub: loaded.userSub,
          tenantOrgId: loaded.tenantOrgId,
          displayName: loaded.displayName,
          emailVerified: loaded.emailVerified,
          accessToken,
        };
      } else {
        event.cookies.delete(SESSION_COOKIE, { path: "/" });
      }
    } catch (e) {
      console.error("[hooks] session load error:", e instanceof Error ? e.message : e);
    }
  }

  return resolve(event);
};

// ── Security headers ───────────────────────────────────────────────────────────

const security: Handle = async ({ event, resolve }) => {
  const response = await resolve(event);

  const zitadelOrigin = getZitadelOrigin();
  const connectSrc = zitadelOrigin ? `'self' ${zitadelOrigin}` : `'self'`;

  const csp = [
    "default-src 'self'",
    "script-src 'self'",
    `connect-src ${connectSrc}`,
    "frame-ancestors 'none'",
    "object-src 'none'",
    "base-uri 'self'",
    "form-action 'self'",
  ].join("; ");

  response.headers.set("Content-Security-Policy", csp);
  response.headers.set("X-Frame-Options", "DENY");
  response.headers.set("X-Content-Type-Options", "nosniff");
  response.headers.set("Referrer-Policy", "strict-origin-when-cross-origin");
  response.headers.set(
    "Permissions-Policy",
    "camera=(), microphone=(), geolocation=(), payment=()"
  );

  const isProd = env.APP_ENV !== "dev" && env.NODE_ENV !== "development";
  if (isProd) {
    response.headers.set(
      "Strict-Transport-Security",
      "max-age=31536000; includeSubDomains"
    );
  }

  return response;
};

export const handle = sequence(session, security);

// ── Helpers ────────────────────────────────────────────────────────────────────

/** Extract JWT `exp` (ms epoch) without verifying signature. Returns null on any error. */
function getJwtExpMs(token: string): number | null {
  try {
    const parts = token.split(".");
    if (parts.length < 2) return null;
    const pad = (s: string) => s + "=".repeat((4 - (s.length % 4)) % 4);
    const payload = JSON.parse(Buffer.from(pad(parts[1]), "base64url").toString("utf8"));
    const exp = payload.exp;
    if (typeof exp !== "number") return null;
    return exp * 1000;
  } catch {
    return null;
  }
}

let _zitadelOrigin: string | null | undefined = undefined;

function getZitadelOrigin(): string | null {
  if (_zitadelOrigin !== undefined) return _zitadelOrigin;
  try {
    const issuer = env.ZITADEL_ISSUER;
    _zitadelOrigin = issuer ? new URL(issuer).origin : null;
  } catch {
    _zitadelOrigin = null;
  }
  return _zitadelOrigin;
}
