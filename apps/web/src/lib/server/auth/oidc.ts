/**
 * OIDC discovery + client configuration for the SvelteKit BFF.
 *
 * Strict validation mirrors the backend Phase 0 invariants:
 * - issuer exact-string equality
 * - HTTPS in production
 * - RS256 in id_token_signing_alg_values_supported
 * - discovery host must match ZITADEL_ISSUER
 *
 * Discovery is cached per process; once fetched it is immutable.
 */
import * as client from "openid-client";
import { env } from "$env/dynamic/private";

export interface OidcConfig {
  issuer: string;
  webClientId: string;
  redirectUri: string;
  postLogoutRedirectUri: string;
  serverConfig: client.Configuration;
}

let _config: OidcConfig | null = null;

function requireEnv(name: string): string {
  const v = env[name];
  if (!v) throw new Error(`Missing required env var: ${name}`);
  return v;
}

function isProd(): boolean {
  return env.APP_ENV !== "dev" && env.APP_ENV !== "development" && env.NODE_ENV !== "development";
}

export async function getOidcConfig(): Promise<OidcConfig> {
  if (_config) return _config;

  const issuer = requireEnv("ZITADEL_ISSUER");
  const webClientId = requireEnv("ZITADEL_WEB_CLIENT_ID");
  const webHost = requireEnv("WEB_HOST");

  if (isProd() && !issuer.startsWith("https://")) {
    throw new Error(`ZITADEL_ISSUER must use HTTPS in production, got: ${issuer}`);
  }

  const protocol = isProd() ? "https" : (env.WEB_PROTOCOL ?? "https");
  const redirectUri = `${protocol}://${webHost}/auth/callback`;
  const postLogoutRedirectUri = `${protocol}://${webHost}/`;

  // openid-client v6 discovery — validates issuer exact-match internally.
  // allowInsecureRequests is required for local HTTP Zitadel in dev mode.
  // discovery(server, clientId, metadata, clientAuthentication, options) — 5 args.
  const discoveryOptions: Parameters<typeof client.discovery>[4] = {};
  if (!isProd()) {
    discoveryOptions.execute = [client.allowInsecureRequests];
  }
  const serverConfig = await client.discovery(
    new URL(issuer),
    webClientId,
    {},
    client.None(),
    discoveryOptions
  );

  // Assert RS256 is in the supported alg list
  const supportedAlgs = serverConfig.serverMetadata().id_token_signing_alg_values_supported ?? [];
  if (!supportedAlgs.includes("RS256")) {
    throw new Error(
      `IdP does not support RS256 for id_token signing; got: ${JSON.stringify(supportedAlgs)}`
    );
  }

  _config = { issuer, webClientId, redirectUri, postLogoutRedirectUri, serverConfig };
  return _config;
}

/** Reset the cached config (test only). */
export function _resetOidcConfig(): void {
  _config = null;
}
