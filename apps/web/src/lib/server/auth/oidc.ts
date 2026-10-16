/**
 * Zitadel OIDC client for the backend-for-frontend.
 *
 * Discovery runs once per process (a failed discovery is retried on the next
 * call). openid-client validates the issuer string exactly, the ID token's
 * `iss`/`aud`/`exp`/`nonce`, `state` and the PKCE verifier.
 */
import * as client from 'openid-client';
import { config } from '../config.js';

let discovered: Promise<client.Configuration> | null = null;

export function oidc(): Promise<client.Configuration> {
	if (!discovered) {
		const cfg = config();
		const auth = cfg.webClientSecret ? client.ClientSecretPost(cfg.webClientSecret) : client.None();
		const options: client.DiscoveryRequestOptions = { timeout: 10 };
		if (cfg.issuer.protocol === 'http:') {
			// Loopback issuer only (config.ts refuses any other http issuer) — local mock IdP for e2e.
			options.execute = [client.allowInsecureRequests];
		}
		discovered = client.discovery(cfg.issuer, cfg.webClientId, undefined, auth, options).catch((e) => {
			discovered = null;
			throw e;
		});
	}
	return discovered;
}

/**
 * Scopes requested at login. `offline_access` yields the refresh token the
 * session lives on. The Zitadel app must issue JWT access tokens (the gateway
 * verifies them against the issuer's JWKS).
 */
// The resource-owner scope puts the org id (the gateway's tenant) into the tokens; the
// projects:roles scope adds the project roles the gateway maps to admin and super_admin.
export const LOGIN_SCOPE =
	'openid profile email offline_access urn:zitadel:iam:user:resourceowner urn:zitadel:iam:org:projects:roles';

export { client };
