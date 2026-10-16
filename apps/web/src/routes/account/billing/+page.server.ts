import { redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types.js';
import { gatewayFetch, requireSession } from '$lib/server/gateway.js';

export const load: PageServerLoad = async ({ locals }) => {
	const session = requireSession(locals, '/account/billing');
	const fetch = gatewayFetch(session);

	const [plansRes, subscriptionRes] = await Promise.allSettled([
		fetch('/v1/billing/plans'),
		fetch('/v1/billing/subscription'),
	]);

	const plans =
		plansRes.status === 'fulfilled' && plansRes.value.ok
			? await plansRes.value.json()
			: [];
	const subscription =
		subscriptionRes.status === 'fulfilled' && subscriptionRes.value.ok
			? await subscriptionRes.value.json()
			: null;

	return { user: session.user, plans, subscription };
};

export const actions: Actions = {
	upgrade: async ({ request, locals, url }) => {
		const fetch = gatewayFetch(requireSession(locals, '/account/billing'));
		const data = await request.formData();
		const planKey = data.get('plan_key') as string;
		const returnUrl = `${url.origin}/account/billing`;

		const res = await fetch('/v1/billing/subscriptions', {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ plan_key: planKey, return_url: returnUrl }),
		});

		if (!res.ok) {
			return { error: 'Failed to create checkout session' };
		}

		const { url: checkoutUrl } = await res.json();
		redirect(302, checkoutUrl);
	},

	cancel: async ({ locals }) => {
		const fetch = gatewayFetch(requireSession(locals, '/account/billing'));
		await fetch('/v1/billing/subscription', { method: 'DELETE' });
		redirect(302, '/account/billing');
	},

	portal: async ({ locals, url }) => {
		const fetch = gatewayFetch(requireSession(locals, '/account/billing'));
		const returnUrl = `${url.origin}/account/billing`;
		const res = await fetch('/v1/billing/portal', {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ return_url: returnUrl }),
		});
		if (!res.ok) return { error: 'Could not open billing portal' };
		const { url: portalUrl } = await res.json();
		redirect(302, portalUrl);
	},
};
