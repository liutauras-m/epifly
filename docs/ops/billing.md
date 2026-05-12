# Billing Operations Runbook

## Services

| Service | URL | Purpose |
|---------|-----|---------|
| Lago API | `http://localhost:3010` | Subscription / event API |
| Lago UI | `http://localhost:3010/ui` | Admin dashboard |
| Zitadel | `http://localhost:8085` | Identity / OIDC |
| Stripe | `https://dashboard.stripe.com` | Payments / cards |

---

## Rotating Stripe keys

1. Generate a new key in the Stripe dashboard.
2. Update `STRIPE_SECRET_KEY` in Lago's environment (docker-compose `.env`).
3. Restart Lago worker: `docker compose --profile infra restart lago-worker`.
4. Update `STRIPE_PUBLIC_KEY` in the SvelteKit `.env`.
5. Verify a test checkout completes.

---

## Replaying a failed Lago webhook

```bash
# Find the event ID in Lago admin UI, then:
curl -X POST https://<lago-host>/api/v1/webhooks/<event_id>/retry \
  -H "Authorization: Bearer $LAGO_API_KEY"
```

Lago retries automatically for up to 72 hours with exponential backoff.

---

## Issuing credits to a tenant

Via Lago admin UI: **Customers → <tenant_id> → Credits → Add**.

Via API:
```bash
curl -X POST https://<lago-host>/api/v1/wallet_transactions \
  -H "Authorization: Bearer $LAGO_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"wallet_transaction":{"wallet_id":"<wallet_id>","paid_credits":"10","granted_credits":"0"}}'
```

---

## Cancelling a subscription for a tenant

Via gateway API (requires super-admin JWT):
```bash
curl -X DELETE https://<gateway>/v1/billing/subscription \
  -H "Authorization: Bearer <super_admin_jwt>"
```

Or directly in Lago UI: **Subscriptions → <sub_id> → Terminate**.

---

## Looking up a tenant by ID

Gateway: `GET /v1/billing/subscription` (authenticated as the tenant).

Lago:
```bash
curl https://<lago-host>/api/v1/customers/<tenant_id> \
  -H "Authorization: Bearer $LAGO_API_KEY"
```

---

## Drift reconciliation (Lago vs Zitadel)

A nightly drift check can be run manually:
```bash
# List Zitadel orgs without a Lago subscription
tsx scripts/migrate-to-zitadel-lago.ts < tenant_ids.jsonl
```

---

## Webhook signature verification

Gateway verifies `X-Lago-Signature` header using HMAC-SHA256 with `LAGO_WEBHOOK_SECRET`.
If verification fails, the gateway returns `401` and logs the error.

To test a webhook locally:
```bash
SECRET=your_webhook_secret
PAYLOAD='{"webhook_type":"subscription.started","object":{}}'
SIG=$(echo -n "$PAYLOAD" | openssl dgst -sha256 -hmac "$SECRET" | awk '{print $2}')
curl -X POST http://localhost:8080/v1/billing/webhooks \
  -H "X-Lago-Signature: $SIG" \
  -H "Content-Type: application/json" \
  -d "$PAYLOAD"
```
