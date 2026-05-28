import type { Handle } from "@sveltejs/kit";
import { loadSession, refreshSession } from "$lib/server/auth/session";
import { getOidcConfig } from "$lib/server/auth/oidc";
import * as client from "openid-client";

const SESSION_COOKIE = "__Host-epifly_sid";
/** Refresh the access token when it expires within this many ms. */
const REFRESH_THRESHOLD_MS = 60_000;

export const handle: Handle = async ({ event, resolve }) => {
  const sid = event.cookies.get(SESSION_COOKIE);

  if (sid) {
    try {
      const session = await loadSession(sid);

      if (session) {
        // Check if the access token is close to expiry — if so, refresh.
        // We need the raw row to inspect access_expires_at; loadSession
        // returns the decrypted access_token but not the expiry directly.
        // We use a lightweight approach: attempt refresh via refreshSession
        // which does the SELECT FOR UPDATE and only calls the IdP if needed.
        let accessToken = session.accessToken;

        // Try to parse expiry from the JWT `exp` claim to avoid an extra DB round-trip.
        // If parsing fails, we skip the proactive refresh (the proxy will get a 401 and retry).
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
                  refreshToken: (tokenSet.refresh_token ?? oldRefreshToken),
                  accessExpiresAt: new Date(Date.now() + exp * 1000),
                };
              },
            });

            if (rotated === null) {
              // Session was revoked (invalid_grant)
              event.cookies.delete(SESSION_COOKIE, { path: "/" });
            } else {
              accessToken = rotated;
            }
          }
        } catch {
          // Non-fatal: skip proactive refresh; the proxy will handle 401s
        }

        event.locals.session = {
          userIss: session.userIss,
          userSub: session.userSub,
          tenantOrgId: session.tenantOrgId,
          displayName: session.displayName,
          emailVerified: session.emailVerified,
          accessToken,
        };
      } else {
        // Session not found or revoked
        event.cookies.delete(SESSION_COOKIE, { path: "/" });
      }
    } catch (e) {
      console.error("[hooks] session load error:", e instanceof Error ? e.message : e);
    }
  }

  return resolve(event);
};

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
