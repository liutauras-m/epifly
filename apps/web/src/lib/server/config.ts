/**
 * Web server configuration — the complete environment contract of the
 * SvelteKit backend-for-frontend. Read once in the `init` hook; a missing or
 * malformed variable stops the server before it accepts a request.
 *
 *   ORIGIN                     public origin of this app (also read by adapter-node)
 *   GATEWAY_URL                internal gateway URL, e.g. http://gateway:8080
 *   ZITADEL_ISSUER             OIDC issuer URL (https; http only for a loopback host)
 *   ZITADEL_WEB_CLIENT_ID      Zitadel web application client id (PKCE)
 *   ZITADEL_WEB_CLIENT_SECRET  optional; set only for a confidential client
 *   AUTH_SESSION_KEY           32 random bytes, base64 — AES-256-GCM session key
 */
import { env } from '$env/dynamic/private';

export interface WebConfig {
	origin: URL;
	gatewayUrl: URL;
	issuer: URL;
	webClientId: string;
	webClientSecret: string | null;
	sessionKey: Buffer;
	/** `${ORIGIN}/auth/callback` — the only redirect URI registered in Zitadel. */
	redirectUri: string;
	/** `${ORIGIN}/auth/signed-out` — registered as the post-logout redirect URI. */
	postLogoutRedirectUri: string;
}

let current: WebConfig | null = null;

function parseUrl(name: string, value: string): URL {
	try {
		return new URL(value);
	} catch {
		throw new Error(`[config] ${name} is not a valid URL`);
	}
}

function isLoopback(url: URL): boolean {
	// RFC 6761: `localhost` and every `*.localhost` name resolve to loopback.
	const host = url.hostname;
	return host === 'localhost' || host.endsWith('.localhost') || host === '127.0.0.1' || host === '[::1]';
}

/** Parse and validate the environment. Throws with the offending variable name. */
export function loadConfig(source: Record<string, string | undefined> = env): WebConfig {
	const get = (name: string) => {
		const value = source[name]?.trim();
		if (!value) throw new Error(`[config] ${name} is required`);
		return value;
	};

	const origin = parseUrl('ORIGIN', get('ORIGIN'));
	if (origin.pathname !== '/' || origin.search || origin.hash) {
		throw new Error('[config] ORIGIN must be a bare origin such as https://app.example.com');
	}
	if (origin.protocol !== 'https:' && !isLoopback(origin)) {
		throw new Error('[config] ORIGIN must use https (http is accepted only for a loopback host)');
	}

	const gatewayUrl = parseUrl('GATEWAY_URL', get('GATEWAY_URL'));
	if (gatewayUrl.protocol !== 'http:' && gatewayUrl.protocol !== 'https:') {
		throw new Error('[config] GATEWAY_URL must be an http(s) URL');
	}

	const issuer = parseUrl('ZITADEL_ISSUER', get('ZITADEL_ISSUER'));
	if (issuer.protocol !== 'https:' && !isLoopback(issuer)) {
		throw new Error('[config] ZITADEL_ISSUER must use https (http is accepted only for a loopback host)');
	}

	const sessionKey = Buffer.from(get('AUTH_SESSION_KEY'), 'base64');
	if (sessionKey.length !== 32) {
		throw new Error('[config] AUTH_SESSION_KEY must be 32 bytes, base64-encoded (openssl rand -base64 32)');
	}

	const base = origin.origin;
	return {
		origin,
		gatewayUrl,
		issuer,
		webClientId: get('ZITADEL_WEB_CLIENT_ID'),
		webClientSecret: source.ZITADEL_WEB_CLIENT_SECRET?.trim() || null,
		sessionKey,
		redirectUri: `${base}/auth/callback`,
		postLogoutRedirectUri: `${base}/auth/signed-out`,
	};
}

/** Validate the environment and keep the result. Called from the `init` hook. */
export function initConfig(): WebConfig {
	current = loadConfig();
	return current;
}

/** The validated configuration. The `init` hook guarantees it exists. */
export function config(): WebConfig {
	if (!current) current = loadConfig();
	return current;
}
