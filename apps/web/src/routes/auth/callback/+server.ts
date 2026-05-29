import { redirect, error } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import * as client from "openid-client";
import type { IDToken } from "openid-client";
import { getOidcConfig } from "$lib/server/auth/oidc";
import { consumeOidcTransaction, createSession } from "$lib/server/auth/session";
import { checkRateLimit, rateLimitKey } from "$lib/server/auth/rate-limit";
import { auditLoginSuccess, auditLoginFailure } from "$lib/server/auth/audit";
import { env } from "$env/dynamic/private";

const ORG_CLAIM = env.ZITADEL_ORG_CLAIM ?? "urn:zitadel:iam:user:resourceowner:id";

export const GET: RequestHandler = async ({ url, cookies, request }) => {
  const rawIp =
    request.headers.get("cf-connecting-ip") ??
    request.headers.get("x-forwarded-for")?.split(",")[0] ??
    undefined;
  const rawUa = request.headers.get("user-agent") ?? undefined;

  // Rate limit callbacks to prevent replay-storm abuse
  const key = rateLimitKey(request.headers);
  if (!checkRateLimit(`cb:${key}`, 20)) {
    auditLoginFailure({ reason: "rate_limited", rawIp, rawUa });
    throw error(429, "too_many_requests");
  }

  const cfg = await getOidcConfig();

  const txState = cookies.get("__Host-epifly_oidc_tx");
  if (!txState) {
    auditLoginFailure({ reason: "missing_oidc_transaction", rawIp, rawUa });
    throw error(400, "missing_oidc_transaction");
  }

  const tx = await consumeOidcTransaction(txState);
  if (!tx) {
    auditLoginFailure({ reason: "transaction_already_consumed", rawIp, rawUa });
    throw error(400, "transaction_already_consumed");
  }

  const returnedState = url.searchParams.get("state");
  if (!returnedState || returnedState !== tx.state) {
    auditLoginFailure({ reason: "state_mismatch", rawIp, rawUa });
    throw error(400, "state_mismatch");
  }

  let tokenSet: client.TokenEndpointResponse & { claims(): IDToken | undefined };
  try {
    tokenSet = (await client.authorizationCodeGrant(
      cfg.serverConfig,
      url,
      {
        pkceCodeVerifier: tx.codeVerifier,
        expectedNonce: tx.nonce,
        expectedState: tx.state,
      },
      { redirect_uri: cfg.redirectUri }
    )) as client.TokenEndpointResponse & { claims(): IDToken | undefined };
  } catch (e) {
    const reason = e instanceof Error ? e.message : "exchange_failed";
    // Never log the full error message — it may contain grant details
    auditLoginFailure({ reason: "exchange_failed", rawIp, rawUa });
    throw redirect(302, `/auth/error?reason=exchange_failed`);
  }

  const { access_token, refresh_token, id_token, expires_in } = tokenSet;
  if (!access_token || !refresh_token) {
    auditLoginFailure({ reason: "missing_tokens", rawIp, rawUa });
    throw redirect(302, `/auth/error?reason=missing_tokens`);
  }

  const idTokenClaims: IDToken | undefined = tokenSet.claims();
  if (!idTokenClaims) {
    auditLoginFailure({ reason: "missing_claims", rawIp, rawUa });
    throw redirect(302, `/auth/error?reason=missing_claims`);
  }

  // Zitadel puts resourceowner:id in the access token JWT, not in the userinfo endpoint.
  // Decode the access token payload (trusted: it was just issued by Zitadel via PKCE exchange).
  let accessTokenClaims: Record<string, unknown> = {};
  try {
    const [, payloadB64] = access_token.split(".");
    if (payloadB64) {
      accessTokenClaims = JSON.parse(
        Buffer.from(payloadB64, "base64url").toString("utf8")
      ) as Record<string, unknown>;
    }
  } catch {
    // Non-JWT access token (opaque) — no Zitadel-specific claims available
  }

  // Merge: ID token claims are authoritative for identity (sub, iss); access token
  // claims supply Zitadel-specific extras (resourceowner:id, org roles, etc.).
  const claims = {
    ...accessTokenClaims,
    ...idTokenClaims,
  } as IDToken & Record<string, unknown>;

  const tenantOrgId = (claims[ORG_CLAIM] as string | undefined) ?? "";
  if (!tenantOrgId) {
    auditLoginFailure({ reason: "missing_org_claim", rawIp, rawUa });
    throw redirect(302, `/auth/error?reason=missing_org_claim`);
  }

  const userIss = String(claims.iss ?? cfg.issuer);
  const userSub = String(claims.sub ?? "");
  if (!userSub) {
    auditLoginFailure({ reason: "missing_sub", rawIp, rawUa });
    throw redirect(302, `/auth/error?reason=missing_sub`);
  }

  const accessExpiresAt = new Date(Date.now() + (expires_in ?? 3600) * 1000);

  const sessionId = await createSession({
    userIss,
    userSub,
    tenantOrgId,
    accessToken: access_token,
    refreshToken: refresh_token,
    idToken: id_token,
    accessExpiresAt,
  });

  auditLoginSuccess({ iss: userIss, sub: userSub, orgId: tenantOrgId, rawIp, rawUa });

  cookies.delete("__Host-epifly_oidc_tx", { path: "/" });
  cookies.set("__Host-epifly_sid", sessionId, {
    httpOnly: true,
    secure: true,
    sameSite: "lax",
    path: "/",
    maxAge: 60 * 60 * 24 * 30,
  });

  throw redirect(302, tx.returnTo || "/");
};
