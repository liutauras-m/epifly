import { redirect } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import * as client from "openid-client";
import { getOidcConfig } from "$lib/server/auth/oidc";
import { revokeSession, loadSession } from "$lib/server/auth/session";
import { auditLogout } from "$lib/server/auth/audit";

export const GET: RequestHandler = async ({ cookies }) => {
  const cfg = await getOidcConfig();
  const sid = cookies.get("__Host-epifly_sid");

  let idToken: string | null = null;

  if (sid) {
    // Load session identity before revocation for the audit event
    const loaded = await loadSession(sid).catch(() => null);

    const revoked = await revokeSession(sid);

    if (revoked) {
      idToken = revoked.idToken;

      if (loaded) {
        auditLogout({ iss: loaded.userIss, sub: loaded.userSub });
      }

      // Best-effort: revoke the refresh token at Zitadel (2s timeout)
      if (revoked.refreshToken) {
        try {
          const controller = new AbortController();
          const timeout = setTimeout(() => controller.abort(), 2000);
          await client.tokenRevocation(cfg.serverConfig, revoked.refreshToken);
          clearTimeout(timeout);
        } catch (e) {
          console.warn(
            "[auth/logout] refresh token revocation best-effort failed:",
            e instanceof Error ? e.message : "unknown"
          );
        }
      }
    }
  }

  cookies.delete("__Host-epifly_sid", { path: "/" });
  cookies.delete("__Host-epifly_oidc_tx", { path: "/auth/callback" });

  const metadata = cfg.serverConfig.serverMetadata();
  const endSessionUrl = metadata.end_session_endpoint;

  if (endSessionUrl) {
    const url = new URL(endSessionUrl);
    url.searchParams.set("post_logout_redirect_uri", cfg.postLogoutRedirectUri);
    if (idToken) url.searchParams.set("id_token_hint", idToken);
    throw redirect(302, url.href);
  }

  throw redirect(302, cfg.postLogoutRedirectUri);
};
