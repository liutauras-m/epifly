import { redirect, error } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import * as client from "openid-client";
import { getOidcConfig } from "$lib/server/auth/oidc";
import { createOidcTransaction } from "$lib/server/auth/session";
import { env } from "$env/dynamic/private";

// Allowed returnTo paths: same-origin only, must start with /
const RETURN_TO_RE = /^\/(?!\/)[^?#]*(?:\?[^#]*)?(?:#.*)?$/;

function sanitizeReturnTo(raw: string | null): string {
  if (!raw) return "/";
  // Reject anything that looks like an absolute URL or protocol-relative
  if (!RETURN_TO_RE.test(raw)) return "/";
  return raw;
}

export const GET: RequestHandler = async ({ url, cookies }) => {
  const cfg = await getOidcConfig();
  const returnTo = sanitizeReturnTo(url.searchParams.get("returnTo"));

  const codeVerifier = client.randomPKCECodeVerifier();
  const codeChallenge = await client.calculatePKCECodeChallenge(codeVerifier);
  const state = client.randomState();
  const nonce = client.randomNonce();

  // Persist transaction row — single-use, expires after 1 day (cleaned by cron)
  await createOidcTransaction({ state, codeVerifier, nonce, returnTo });

  // Short-lived tx cookie so the callback can retrieve the state/nonce
  cookies.set("__Host-epifly_oidc_tx", state, {
    httpOnly: true,
    secure: true,
    sameSite: "lax",
    path: "/auth/callback",
    maxAge: 600, // 10 minutes
  });

  const isProd = env.APP_ENV !== "dev" && env.NODE_ENV !== "development";
  const authUrl = client.buildAuthorizationUrl(cfg.serverConfig, {
    redirect_uri: cfg.redirectUri,
    scope: "openid profile email",
    state,
    nonce,
    code_challenge: codeChallenge,
    code_challenge_method: "S256",
    ...(isProd ? {} : {}),
  });

  throw redirect(302, authUrl.href);
};
