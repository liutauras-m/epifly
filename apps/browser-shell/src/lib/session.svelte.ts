/**
 * Shell session state. Sign-in runs in Rust (system browser + loopback PKCE,
 * see src-tauri/src/auth); this module only mirrors who is signed in. No
 * token is ever stored in the webview — not in localStorage, not in cookies.
 *
 * Module-level state is fine here: the shell is a client-only SPA (ssr = false).
 */
import { invoke } from '@tauri-apps/api/core';

export interface ShellUser {
	sub: string;
	name: string;
	email: string | null;
}

class ShellSession {
	user = $state<ShellUser | null>(null);
	/** 'loading' until the vault was checked once. */
	status = $state<'loading' | 'signed-out' | 'signing-in' | 'signed-in'>('loading');
	error = $state<string | null>(null);
}

export const session = new ShellSession();

function errorMessage(e: unknown): string {
	return typeof e === 'string' ? e : e instanceof Error ? e.message : 'Sign-in failed';
}

/** Restore the signed-in user from the Rust vault (no network). */
export async function restoreSession(): Promise<void> {
	try {
		session.user = await invoke<ShellUser | null>('auth_status');
		session.status = session.user ? 'signed-in' : 'signed-out';
	} catch (e) {
		session.error = errorMessage(e);
		session.status = 'signed-out';
	}
}

/** Open the system browser for Zitadel sign-in and wait for the loopback callback. */
export async function signIn(): Promise<void> {
	session.error = null;
	session.status = 'signing-in';
	try {
		session.user = await invoke<ShellUser>('auth_sign_in');
		session.status = 'signed-in';
	} catch (e) {
		session.error = errorMessage(e);
		session.status = 'signed-out';
	}
}

/** Revoke the refresh token, clear the vault and end the IdP browser session. */
export async function signOut(): Promise<void> {
	try {
		await invoke('auth_sign_out');
	} finally {
		session.user = null;
		session.status = 'signed-out';
	}
}

/** Called when the gateway or Rust reports the session is gone. */
export function sessionExpired(message = 'Your session expired. Please sign in again.'): void {
	session.user = null;
	session.error = message;
	session.status = 'signed-out';
}
