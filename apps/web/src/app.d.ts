import type { Session } from '$lib/server/auth/session.js';

declare global {
	namespace App {
		interface Locals {
			/** Decrypted, refreshed session — null when signed out. Never sent to the browser. */
			session: Session | null;
		}
	}
}

export {};
