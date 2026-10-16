# Billing Operations Runbook

## Services

| Service | Where | Purpose |
|---------|-------|---------|
| Lago API (`lago-api`) | `https://<BILLING_HOST>`; locally `http://localhost:3001` | Subscription / event API |
| Zitadel (`zitadel`) | `https://<AUTH_HOST>`; locally `http://localhost:8085` | Identity / OIDC |
| Gateway | `/v1/billing/*`, `/admin/billing/*` | Plans, subscriptions, invoices, usage; webhook receiver |

The compose project runs `lago-api`, `lago-worker`, `lago-clock` and the one-shot
`lago-migrate` (Lago v1.55.0); it does not run Lago's front-end. Billing is on in the
gateway only when `LAGO_API_KEY` is set; then every tenant's plan comes from its Lago
subscription (cached for 5 minutes, Free without an active subscription). Setting the
keys up: [deploy-conusai-cloud.md](../deploy-conusai-cloud.md).

---

## Payment provider keys

Stripe (or another payment provider) is configured inside Lago as an integration, not in
the epifly environment. Rotate its keys there.

---

## Replaying a failed Lago webhook

```bash
# With the webhook id from Lago, then:
curl -X POST https://<BILLING_HOST>/api/v1/webhooks/<event_id>/retry \
  -H "Authorization: Bearer $LAGO_API_KEY"
```

Lago retries automatically for up to 72 hours with exponential backoff.

---

## Issuing credits to a tenant

Via the gateway (super-admin bearer token):
```bash
curl -X POST https://<gateway>/admin/billing/credits \
  -H "Authorization: Bearer <super_admin_access_token>" \
  -H "Content-Type: application/json" \
  -d '{"tenant_id":"<zitadel_org_id>","amount_cents":1000,"description":"goodwill"}'
```

Via the Lago API:
```bash
curl -X POST https://<BILLING_HOST>/api/v1/wallet_transactions \
  -H "Authorization: Bearer $LAGO_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"wallet_transaction":{"wallet_id":"<wallet_id>","paid_credits":"10","granted_credits":"0"}}'
```

---

## Cancelling a subscription for a tenant

As a super-admin, for any tenant:
```bash
curl -X POST https://<gateway>/admin/billing/cancel/<zitadel_org_id> \
  -H "Authorization: Bearer <super_admin_access_token>"
```

A tenant cancels its own subscription with `DELETE /v1/billing/subscription`.

---

## Looking up a tenant by ID

The tenant id is the Zitadel organization id; it is also the Lago customer `external_id`.

Gateway: `GET /v1/billing/subscription` (authenticated as the tenant).

Lago:
```bash
curl https://<BILLING_HOST>/api/v1/customers/<tenant_id> \
  -H "Authorization: Bearer $LAGO_API_KEY"
```

---

## Reconciliation

The scheduled job `lago-reconcile` (`apps/backend/crates/jobs/src/jobs/lago_reconcile.rs`,
daily at 02:00 UTC) logs Lago's analytics summary and warns on anomalies; it does not yet
compare Zitadel organizations with Lago customers. Run it on demand with
`POST /admin/jobs/lago-reconcile/run` (super-admin). `scripts/migrate-to-zitadel-lago.ts`
is a one-off migration that creates Zitadel organizations and Lago customers for a list of
tenant ids.

---

## Webhook signature verification

Lago must sign webhooks with HMAC (not JWT). The gateway verifies the `X-Lago-Signature`
header, the Base64 HMAC-SHA256 of the raw body keyed with `LAGO_WEBHOOK_SECRET` (the
organization's HMAC key from Lago). A missing or wrong signature, or
`X-Lago-Signature-Algorithm` other than `hmac`, returns `401`; without billing configured the
route returns `503`. Replays of a seen `webhook_id` are acknowledged and ignored.

To test a webhook locally:
```bash
SECRET=$LAGO_WEBHOOK_SECRET
PAYLOAD='{"webhook_type":"subscription.started","object":{}}'
SIG=$(printf %s "$PAYLOAD" | openssl dgst -sha256 -hmac "$SECRET" -binary | base64)
curl -X POST http://localhost:8080/v1/billing/webhooks \
  -H "X-Lago-Signature: $SIG" \
  -H "Content-Type: application/json" \
  -d "$PAYLOAD"
```
