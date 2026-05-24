import type { SessionUser } from '$lib/server/session.js';

declare global {
	namespace App {
		interface Locals {
			user: SessionUser | null;
		}
	}
}

export {};
