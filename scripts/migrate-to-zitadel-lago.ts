#!/usr/bin/env tsx
/**
 * Migration script: move existing tenants from redb to Zitadel + Lago.
 *
 * Prerequisites:
 *   ZITADEL_DOMAIN, ZITADEL_MGMT_PAT, LAGO_API_URL, LAGO_API_KEY must be set.
 *   The gateway REDB_PATH must be readable.
 *
 * Run:
 *   REDB_PATH=/data/conusai.redb tsx scripts/migrate-to-zitadel-lago.ts
 */

interface TenantRecord {
	id: string;
	name?: string;
	email?: string;
	plan?: string;
}

async function listTenantsFromEnv(): Promise<TenantRecord[]> {
	// In the absence of a redb Node binding, this script reads tenant IDs
	// from the MIGRATION_TENANT_IDS env var (comma-separated) or stdin.
	const raw = process.env.MIGRATION_TENANT_IDS ?? '';
	if (raw) {
		return raw.split(',').map((id) => ({ id: id.trim() }));
	}

	// Read JSON lines from stdin: { id, name, email, plan }
	const lines: TenantRecord[] = [];
	for await (const line of process.stdin) {
		const text = line.toString().trim();
		if (!text) continue;
		try {
			lines.push(JSON.parse(text));
		} catch {
			console.warn(`Skipping invalid JSON line: ${text}`);
		}
	}
	return lines;
}

async function ensureZitadelOrg(
	domain: string,
	pat: string,
	name: string
): Promise<string> {
	const url = `${domain}/management/v1/orgs`;
	const res = await fetch(url, {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${pat}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({ name }),
	});
	if (!res.ok) {
		const text = await res.text();
		throw new Error(`Zitadel create org failed: HTTP ${res.status} — ${text}`);
	}
	const data = (await res.json()) as { organizationId?: string };
	return data.organizationId ?? name;
}

async function ensureLagoCustomer(
	apiUrl: string,
	apiKey: string,
	externalId: string,
	email?: string
): Promise<void> {
	const checkUrl = `${apiUrl}/api/v1/customers/${externalId}`;
	const checkRes = await fetch(checkUrl, {
		headers: { Authorization: `Bearer ${apiKey}` },
	});
	if (checkRes.ok) return; // Already exists.

	const createUrl = `${apiUrl}/api/v1/customers`;
	const createRes = await fetch(createUrl, {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${apiKey}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({
			customer: { external_id: externalId, name: externalId, email },
		}),
	});
	if (!createRes.ok) {
		const text = await createRes.text();
		throw new Error(
			`Lago create customer failed: HTTP ${createRes.status} — ${text}`
		);
	}
}

async function ensureLagoSubscription(
	apiUrl: string,
	apiKey: string,
	externalCustomerId: string,
	planCode = 'free'
): Promise<void> {
	const url = `${apiUrl}/api/v1/subscriptions`;
	const res = await fetch(url, {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${apiKey}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({
			subscription: {
				external_id: `${externalCustomerId}-${planCode}`,
				external_customer_id: externalCustomerId,
				plan_code: planCode,
			},
		}),
	});
	// 422 = already exists, acceptable.
	if (!res.ok && res.status !== 422) {
		const text = await res.text();
		console.warn(
			`Lago create subscription warning: HTTP ${res.status} — ${text}`
		);
	}
}

async function main() {
	const zitadelDomain = process.env.ZITADEL_DOMAIN;
	const zitadelPat = process.env.ZITADEL_MGMT_PAT;
	const lagoApiUrl = process.env.LAGO_API_URL ?? 'http://localhost:3010';
	const lagoApiKey = process.env.LAGO_API_KEY;

	if (!zitadelDomain || !zitadelPat || !lagoApiKey) {
		console.error(
			'Missing required env vars: ZITADEL_DOMAIN, ZITADEL_MGMT_PAT, LAGO_API_KEY'
		);
		process.exit(1);
	}

	const tenants = await listTenantsFromEnv();
	console.log(`Migrating ${tenants.length} tenant(s)...`);

	let ok = 0;
	let failed = 0;

	for (const tenant of tenants) {
		try {
			const name = tenant.name ?? tenant.id;

			// 1. Create Zitadel org (idempotent).
			const orgId = await ensureZitadelOrg(zitadelDomain, zitadelPat, name);
			console.log(`  [zitadel] org created: ${tenant.id} → ${orgId}`);

			// 2. Create Lago customer (idempotent).
			await ensureLagoCustomer(lagoApiUrl, lagoApiKey, tenant.id, tenant.email);
			console.log(`  [lago] customer ensured: ${tenant.id}`);

			// 3. Create Lago Free subscription (idempotent).
			await ensureLagoSubscription(lagoApiUrl, lagoApiKey, tenant.id, tenant.plan ?? 'free');
			console.log(`  [lago] subscription ensured: ${tenant.id} → ${tenant.plan ?? 'free'}`);

			ok++;
		} catch (e) {
			console.error(`  ERROR migrating tenant ${tenant.id}: ${e}`);
			failed++;
		}
	}

	console.log(`\nDone: ${ok} succeeded, ${failed} failed.`);
	if (failed > 0) process.exit(1);
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
