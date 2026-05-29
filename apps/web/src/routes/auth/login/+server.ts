import { redirect, error } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import * as client from "openid-client";
import { getOidcConfig } from "$lib/server/auth/oidc";
import { createOidcTransaction } from "$lib/server/auth/session";
import { checkRateLimit, rateLimitKey } from "$lib/server/auth/rate-limit";
import { auditLoginFailure } from "$lib/server/auth/audit";
import { env } from "$env/dynamic/private";

// Allowed returnTo paths: same-origin only, must start with /
const RETURN_TO_RE = /^\/(?!\/)[^?#]*(?:\?[^#]*)?(?:#.*)?$/;

function sanitizeReturnTo(raw: string | null): string {
  if (!raw) return "/";
  if (!RETURN_TO_RE.test(raw)) return "/";
  return raw;
}

export const GET: RequestHandler = async ({ url, cookies, request }) => {
  // Rate limit by IP: 10 attempts per 60s
  const key = rateLimitKey(request.headers);
  if (!checkRateLimit(key)) {
    const rawIp = request.headers.get("cf-connecting-ip") ??
      request.headers.get("x-forwarded-for")?.split(",")[0] ??
      undefined;
    auditLoginFailure({
      reason: "rate_limited",
      rawIp,
      rawUa: request.headers.get("user-agent") ?? undefined,
    });
    throw error(429, "too_many_requests");
  }

  const cfg = await getOidcConfig();
  const returnTo = sanitizeReturnTo(url.searchParams.get("returnTo"));

  const codeVerifier = client.randomPKCECodeVerifier();
  const codeChallenge = await client.calculatePKCECodeChallenge(codeVerifier);
  const state = client.randomState();
  const nonce = client.randomNonce();

  await createOidcTransaction({ state, codeVerifier, nonce, returnTo });

  // __Host- prefix requires Path=/ (RFC 6265bis §4.1.3).
  // The cookie contains only an opaque state ref, so Path=/ is safe.
  cookies.set("__Host-epifly_oidc_tx", state, {
    httpOnly: true,
    secure: true,
    sameSite: "lax",
    path: "/",
    maxAge: 600,
  });

  // Include the Zitadel org scope so the issued access token carries
  // urn:zitadel:iam:user:resourceowner:id — required for tenant derivation (auth invariant 37).
  const orgId = env.ZITADEL_DEFAULT_ORG_ID;
  const scope = ["openid profile email offline_access", orgId ? `urn:zitadel:iam:org:id:${orgId}` : ""]
    .filter(Boolean)
    .join(" ");

  const authUrl = client.buildAuthorizationUrl(cfg.serverConfig, {
    redirect_uri: cfg.redirectUri,
    scope,
    state,
    nonce,
    code_challenge: codeChallenge,
    code_challenge_method: "S256",
  });

  throw redirect(302, authUrl.href);
};
