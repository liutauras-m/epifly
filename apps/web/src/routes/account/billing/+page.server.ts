import { redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types.js';

export const load: PageServerLoad = async ({ locals, fetch }) => {
	if (!locals.user) redirect(302, '/login');

	const [plansRes, subscriptionRes] = await Promise.allSettled([
		fetch('/v1/billing/plans'),
		fetch('/v1/billing/subscription'),
	]);

	const FALLBACK_PLANS = [
		{ key: 'free', display_name: 'Free', monthly_price_cents: 0, max_turns_per_day: 20, max_tokens: 4096, max_storage_gb: 1, rate_limit_rpm: 10 },
		{ key: 'pro', display_name: 'Pro', monthly_price_cents: 2900, max_turns_per_day: 500, max_tokens: 16384, max_storage_gb: 20, rate_limit_rpm: 60 },
		{ key: 'enterprise', display_name: 'Enterprise', monthly_price_cents: 0, max_turns_per_day: null, max_tokens: 128000, max_storage_gb: null, rate_limit_rpm: 600 },
	];

	const plans =
		plansRes.status === 'fulfilled' && plansRes.value.ok
			? await plansRes.value.json()
			: FALLBACK_PLANS;
	const subscription =
		subscriptionRes.status === 'fulfilled' && subscriptionRes.value.ok
			? await subscriptionRes.value.json()
			: null;

	return { user: locals.user, plans, subscription };
};

export const actions: Actions = {
	upgrade: async ({ request, fetch, url }) => {
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

	cancel: async ({ fetch }) => {
		await fetch('/v1/billing/subscription', { method: 'DELETE' });
		redirect(302, '/account/billing');
	},

	portal: async ({ fetch, url }) => {
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
