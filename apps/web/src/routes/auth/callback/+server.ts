import { redirect, error } from "@sveltejs/kit";
import type { RequestHandler } from "./$types";
import * as client from "openid-client";
import type { IDToken } from "openid-client";
import { getOidcConfig } from "$lib/server/auth/oidc";
import { consumeOidcTransaction, createSession } from "$lib/server/auth/session";
import { env } from "$env/dynamic/private";

const ORG_CLAIM = env.ZITADEL_ORG_CLAIM ?? "urn:zitadel:iam:user:resourceowner:id";

export const GET: RequestHandler = async ({ url, cookies }) => {
  const cfg = await getOidcConfig();

  // Retrieve the state from the OIDC tx cookie
  const txState = cookies.get("__Host-epifly_oidc_tx");
  if (!txState) throw error(400, "missing_oidc_transaction");

  // Consume the transaction row (single-use — double-callback → null)
  const tx = await consumeOidcTransaction(txState);
  if (!tx) throw error(400, "transaction_already_consumed");

  // Validate state parameter from IdP matches what we sent
  const returnedState = url.searchParams.get("state");
  if (!returnedState || returnedState !== tx.state) throw error(400, "state_mismatch");

  // Exchange code for tokens
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
    const msg = e instanceof Error ? e.message : String(e);
    console.error("[auth/callback] code exchange failed:", msg);
    throw redirect(302, `/auth/error?reason=exchange_failed`);
  }

  const { access_token, refresh_token, id_token, expires_in } = tokenSet;
  if (!access_token || !refresh_token) throw redirect(302, `/auth/error?reason=missing_tokens`);

  // Extract claims from the verified ID token
  const claims: IDToken | undefined = tokenSet.claims();
  if (!claims) throw redirect(302, `/auth/error?reason=missing_claims`);

  const tenantOrgId = (claims[ORG_CLAIM] as string | undefined) ?? "";
  if (!tenantOrgId) throw redirect(302, `/auth/error?reason=missing_org_claim`);

  const userIss = String(claims.iss ?? cfg.issuer);
  const userSub = String(claims.sub ?? "");
  if (!userSub) throw redirect(302, `/auth/error?reason=missing_sub`);

  const accessExpiresAt = new Date(Date.now() + (expires_in ?? 3600) * 1000);

  // Create session — the fresh session id prevents session fixation
  const sessionId = await createSession({
    userIss,
    userSub,
    tenantOrgId,
    accessToken: access_token,
    refreshToken: refresh_token,
    idToken: id_token,
    accessExpiresAt,
  });

  // Clear tx cookie; set session cookie
  cookies.delete("__Host-epifly_oidc_tx", { path: "/auth/callback" });
  cookies.set("__Host-epifly_sid", sessionId, {
    httpOnly: true,
    secure: true,
    sameSite: "lax",
    path: "/",
    maxAge: 60 * 60 * 24 * 30,
  });

  throw redirect(302, tx.returnTo || "/");
};
