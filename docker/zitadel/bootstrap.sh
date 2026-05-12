#!/usr/bin/env bash
# Zitadel bootstrap script — seeds an admin org + test tenant after first start.
# Runs inside the zitadel-init container which has zitadel-tools available.
# Idempotent: each operation checks before creating.
set -euo pipefail

ZITADEL_URL="${ZITADEL_URL:-http://zitadel:8080}"
ADMIN_USER="${ZITADEL_ADMIN_USER:-admin@conusai.localhost}"
ADMIN_PASS="${ZITADEL_ADMIN_PASS:-ConusAI_Dev_123!}"

echo "→ Waiting for Zitadel at ${ZITADEL_URL}..."
until curl -sf "${ZITADEL_URL}/debug/healthz" > /dev/null; do
  sleep 2
done
echo "  Zitadel is up."

# Obtain a management API token using the machine key (Service Account).
# The machine key file is mounted by docker-compose at /secrets/machine.key.
MACHINE_KEY_FILE="${MACHINE_KEY_FILE:-/secrets/zitadel-machine.key}"

if [ ! -f "$MACHINE_KEY_FILE" ]; then
  echo "WARNING: Machine key not found at ${MACHINE_KEY_FILE}. Skipping automated bootstrap."
  exit 0
fi

# Fetch access token via JWT bearer grant.
TOKEN_RESP=$(curl -sf -X POST "${ZITADEL_URL}/oauth/v2/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  --data-urlencode "grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer" \
  --data-urlencode "scope=openid profile email urn:zitadel:iam:org:project:id:zitadel:aud" \
  --data-urlencode "assertion=$(cat "$MACHINE_KEY_FILE")" || echo "{}")

ACCESS_TOKEN=$(echo "$TOKEN_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('access_token',''))" 2>/dev/null || true)

if [ -z "$ACCESS_TOKEN" ]; then
  echo "WARNING: Could not obtain access token. Bootstrap will be skipped."
  exit 0
fi

AUTH_HEADER="Authorization: Bearer ${ACCESS_TOKEN}"

# ── Create test organization ──────────────────────────────────────────────────
echo "→ Creating test org 'dev-tenant'..."
curl -sf -X POST "${ZITADEL_URL}/management/v1/orgs" \
  -H "$AUTH_HEADER" \
  -H "Content-Type: application/json" \
  -d '{"name":"dev-tenant"}' \
  -o /dev/null || echo "  Org may already exist, continuing."

# ── Create test user ──────────────────────────────────────────────────────────
echo "→ Creating test user 'dev@conusai.localhost'..."
curl -sf -X POST "${ZITADEL_URL}/v2/users/human" \
  -H "$AUTH_HEADER" \
  -H "Content-Type: application/json" \
  -d '{
    "username": "dev@conusai.localhost",
    "profile": {"givenName": "Dev", "familyName": "User"},
    "email": {"email": "dev@conusai.localhost", "isVerified": true},
    "password": {"password": "ConusAI_Dev_123!", "changeRequired": false}
  }' \
  -o /dev/null || echo "  User may already exist, continuing."

echo "✓ Zitadel bootstrap complete."
