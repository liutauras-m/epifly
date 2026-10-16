#!/usr/bin/env node
/**
 * Mock OIDC provider for the web e2e suite — stands in for Zitadel so the
 * real BFF login (authorization code + PKCE + state + nonce) runs end to end
 * without an IdP. Test-only: it approves every authorization request at once
 * and signs ID tokens with a key generated at start-up.
 *
 * Usage: MOCK_OIDC_PORT=4180 node e2e/fixtures/mock-oidc.mjs
 * Web server env: ZITADEL_ISSUER=http://127.0.0.1:4180 ZITADEL_WEB_CLIENT_ID=e2e-web
 */
import { createHash, generateKeyPairSync, randomBytes, sign } from 'node:crypto';
import { createServer } from 'node:http';

const port = Number(process.env.MOCK_OIDC_PORT ?? 4180);
const issuer = `http://127.0.0.1:${port}`;
const { privateKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
const codes = new Map();

const b64url = (v) => Buffer.from(typeof v === 'string' ? v : JSON.stringify(v)).toString('base64url');
function jwt(claims) {
	const head = `${b64url({ alg: 'RS256', typ: 'JWT', kid: 'e2e' })}.${b64url(claims)}`;
	return `${head}.${sign('sha256', Buffer.from(head), privateKey).toString('base64url')}`;
}
function json(res, status, body) {
	res.writeHead(status, { 'content-type': 'application/json', 'cache-control': 'no-store' });
	res.end(JSON.stringify(body));
}
async function form(req) {
	let raw = '';
	for await (const chunk of req) raw += chunk;
	return new URLSearchParams(raw);
}
function tokens(clientId, nonce) {
	const now = Math.floor(Date.now() / 1000);
	const claims = { iss: issuer, sub: 'e2e-user', aud: [clientId], azp: clientId, iat: now, exp: now + 3600 };
	return {
		token_type: 'Bearer',
		expires_in: 3600,
		access_token: jwt({ ...claims, scope: 'openid profile email offline_access' }),
		refresh_token: randomBytes(24).toString('base64url'),
		id_token: jwt({ ...claims, nonce, name: 'E2E Operator', email: 'e2e@example.test' }),
	};
}

createServer(async (req, res) => {
	const url = new URL(req.url ?? '/', issuer);
	if (url.pathname === '/.well-known/openid-configuration') {
		return json(res, 200, {
			issuer,
			authorization_endpoint: `${issuer}/oauth/v2/authorize`,
			token_endpoint: `${issuer}/oauth/v2/token`,
			revocation_endpoint: `${issuer}/oauth/v2/revoke`,
			end_session_endpoint: `${issuer}/oidc/v1/end_session`,
			jwks_uri: `${issuer}/oauth/v2/keys`,
			response_types_supported: ['code'],
			code_challenge_methods_supported: ['S256'],
			id_token_signing_alg_values_supported: ['RS256'],
			token_endpoint_auth_methods_supported: ['none', 'client_secret_post'],
		});
	}
	if (url.pathname === '/oauth/v2/keys') {
		return json(res, 200, { keys: [{ ...privateKey.export({ format: 'jwk' }), d: undefined, p: undefined, q: undefined, dp: undefined, dq: undefined, qi: undefined, kid: 'e2e', use: 'sig', alg: 'RS256' }] });
	}
	if (url.pathname === '/oauth/v2/authorize') {
		const q = url.searchParams;
		if (q.get('code_challenge_method') !== 'S256' || !q.get('state') || !q.get('nonce')) {
			return json(res, 400, { error: 'invalid_request' });
		}
		const code = randomBytes(16).toString('base64url');
		codes.set(code, { challenge: q.get('code_challenge'), nonce: q.get('nonce'), clientId: q.get('client_id'), redirectUri: q.get('redirect_uri') });
		const back = new URL(q.get('redirect_uri'));
		back.searchParams.set('code', code);
		back.searchParams.set('state', q.get('state'));
		res.writeHead(302, { location: back.href });
		return res.end();
	}
	if (url.pathname === '/oauth/v2/token' && req.method === 'POST') {
		const body = await form(req);
		if (body.get('grant_type') === 'refresh_token') return json(res, 200, tokens(body.get('client_id'), undefined));
		const entry = codes.get(body.get('code') ?? '');
		codes.delete(body.get('code') ?? '');
		const verifier = body.get('code_verifier') ?? '';
		if (!entry || entry.redirectUri !== body.get('redirect_uri') || createHash('sha256').update(verifier).digest('base64url') !== entry.challenge) {
			return json(res, 400, { error: 'invalid_grant' });
		}
		return json(res, 200, tokens(entry.clientId, entry.nonce));
	}
	if (url.pathname === '/oauth/v2/revoke') return json(res, 200, {});
	if (url.pathname === '/oidc/v1/end_session') {
		res.writeHead(302, { location: url.searchParams.get('post_logout_redirect_uri') ?? '/' });
		return res.end();
	}
	if (url.pathname === '/healthz') return json(res, 200, { ok: true });
	json(res, 404, { error: 'not_found' });
}).listen(port, '127.0.0.1', () => console.log(`[mock-oidc] ${issuer}`));
