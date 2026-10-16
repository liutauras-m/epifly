#!/usr/bin/env bash
# Print a complete .env for a local run: .env.example with every secret generated.
#   scripts/generate-env.sh > .env && chmod 600 .env
# For ConusAI Cloud, generate the same project variables once and set them on the project;
# never regenerate ZITADEL_MASTERKEY, RUSTFS_IAM_ENC_KEY or the LAGO_ENCRYPTION_* keys of a
# running instance: they encrypt stored data.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v openssl >/dev/null || { echo "openssl is required" >&2; exit 1; }
b64() { openssl rand -base64 "$1" | tr -d '\n'; }
hex() { openssl rand -hex "$1"; }
pg_password=$(hex 24)
declare -A v=(
  [WEB_HOST]=localhost:3000
  [AUTH_HOST]=zitadel.localhost:8085
  [BILLING_HOST]=localhost:3001
  [PLATFORM_ADMIN_TOKEN]=$(b64 32)
  [RUSTFS_IAM_ENC_KEY]=$(b64 32)
  [AUTH_SESSION_KEY]=$(b64 32)
  [ZITADEL_MASTERKEY]=$(hex 16)
  [ZITADEL_DB_PASSWORD]=$(hex 24)
  [ZITADEL_ADMIN_USERNAME]=admin
  [ZITADEL_ADMIN_PASSWORD]="$(hex 12)Aa1!"
  [LAGO_SECRET_KEY_BASE]=$(hex 64)
  [LAGO_ENCRYPTION_PRIMARY_KEY]=$(hex 16)
  [LAGO_ENCRYPTION_DETERMINISTIC_KEY]=$(hex 16)
  [LAGO_ENCRYPTION_KEY_DERIVATION_SALT]=$(hex 16)
  [LAGO_RSA_PRIVATE_KEY]=$(openssl genrsa 2048 2>/dev/null | base64 -w0)
  [POSTGRES_PASSWORD]=$pg_password
  [POSTGRES_URL]="postgresql://epifly:${pg_password}@postgres:5432/epifly"
  [QDRANT_API_KEY]=$(hex 24)
  [AWS_ACCESS_KEY_ID]=$(hex 10)
  [AWS_SECRET_ACCESS_KEY]=$(hex 24)
)
while IFS= read -r line; do
  key=${line%%=*}
  if [[ $line != \#* && $line == *=* && -n ${v[$key]+set} && -z ${line#*=} ]]; then
    printf '%s=%s\n' "$key" "${v[$key]}"
  else
    printf '%s\n' "$line"
  fi
done < .env.example
