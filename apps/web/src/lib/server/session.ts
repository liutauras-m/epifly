import crypto from 'node:crypto';

export const COOKIE_NAME = 'conusai_session';
const TTL_SECS = 24 * 3600;

export interface SessionUser {
	name: string;
	plan: string;
	exp: number;
}

function getKey(): string {
	return process.env.UI_SESSION_KEY ?? 'conusai-foundry-dev-secret-change-me-32b';
}

function b64url(buf: Buffer): string {
	return buf.toString('base64url');
}

export function sign(name: string, plan: string): string {
	const exp = Math.floor(Date.now() / 1000) + TTL_SECS;
	const payload: SessionUser = { name, plan, exp };
	const payloadB64 = b64url(Buffer.from(JSON.stringify(payload)));
	const mac = crypto.createHmac('sha256', getKey()).update(payloadB64).digest();
	return `${payloadB64}.${b64url(mac)}`;
}

export function verify(cookieValue: string): SessionUser | null {
	const dot = cookieValue.indexOf('.');
	if (dot < 0) return null;
	const payloadB64 = cookieValue.slice(0, dot);
	const sigB64 = cookieValue.slice(dot + 1);
	const expected = b64url(crypto.createHmac('sha256', getKey()).update(payloadB64).digest());
	try {
		if (!crypto.timingSafeEqual(Buffer.from(expected), Buffer.from(sigB64))) return null;
	} catch {
		return null;
	}
	try {
		const user = JSON.parse(Buffer.from(payloadB64, 'base64url').toString()) as SessionUser;
		if (user.exp < Math.floor(Date.now() / 1000)) return null;
		return user;
	} catch {
		return null;
	}
}

export function firstName(name: string): string {
	return name.split(/\s+/)[0] ?? name;
}

export function initials(name: string): string {
	return name
		.split(/\s+/)
		.slice(0, 2)
		.map((w) => w[0] ?? '')
		.join('')
		.toUpperCase();
}

export function timeGreeting(): string {
	const h = new Date().getHours();
	if (h >= 5 && h < 12) return 'Morning';
	if (h >= 12 && h < 18) return 'Afternoon';
	if (h >= 18 && h < 23) return 'Evening';
	return 'Late night';
}
