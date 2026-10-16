import type { LayoutServerLoad } from './$types.js';
import { gatewayFetch } from '$lib/server/gateway.js';

function initials(name: string): string {
	return name
		.split(/\s+/)
		.slice(0, 2)
		.map((w) => w[0] ?? '')
		.join('')
		.toUpperCase();
}

/** Only display data leaves the server — never a token. */
export const load: LayoutServerLoad = async ({ locals }) => {
	const session = locals.session;
	if (!session) return { user: null };

	let plan = '';
	try {
		const res = await gatewayFetch(session)('/v1/billing/subscription', { signal: AbortSignal.timeout(3000) });
		if (res.ok) plan = String(((await res.json()) as { plan_key?: string }).plan_key ?? '');
	} catch {
		/* billing unavailable — the plan badge stays empty */
	}

	const { name, email } = session.user;
	return {
		user: {
			name,
			email,
			plan: plan.toUpperCase(),
			firstName: name.split(/\s+/)[0] ?? name,
			initials: initials(name),
		},
	};
};
