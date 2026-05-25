#!/usr/bin/env bash
# Conusai infra verification — run from your laptop to probe the public
# endpoints AND (optionally) SSH into the Dokploy host to inspect containers,
# Traefik labels and network attachment.
#
# Usage:
#   ./verify-infra.sh                          # public probes only
#   ./verify-infra.sh --host root@beta.test.cloud.conusai.com
#   APP_DOMAIN=epifly.beta.test.cloud.conusai.com ./verify-infra.sh
#
# Exits non-zero on any failed check.

set -uo pipefail

APP_DOMAIN="${APP_DOMAIN:-epifly.beta.test.cloud.conusai.com}"
STACK_PREFIX="${STACK_PREFIX:-epifly-infra}"
SSH_HOST=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host) SSH_HOST="$2"; shift 2 ;;
    --domain) APP_DOMAIN="$2"; shift 2 ;;
    --stack) STACK_PREFIX="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# ── pretty printing ─────────────────────────────────────────────────────────
if [[ -t 1 ]]; then
  GREEN=$'\e[32m'; RED=$'\e[31m'; YELLOW=$'\e[33m'; DIM=$'\e[2m'; RESET=$'\e[0m'
else
  GREEN=""; RED=""; YELLOW=""; DIM=""; RESET=""
fi

FAIL=0
pass() { printf '  %sPASS%s %s\n' "$GREEN" "$RESET" "$1"; }
fail() { printf '  %sFAIL%s %s\n' "$RED" "$RESET" "$1"; FAIL=$((FAIL + 1)); }
warn() { printf '  %sWARN%s %s\n' "$YELLOW" "$RESET" "$1"; }
info() { printf '  %s%s%s\n' "$DIM" "$1" "$RESET"; }
section() { printf '\n%s──[ %s ]%s\n' "$DIM" "$1" "$RESET"; }

# ── DNS ─────────────────────────────────────────────────────────────────────
section "DNS"
resolve() {
  # Prefer dig, fall back to host/getent. Returns first IPv4 or empty.
  if command -v dig >/dev/null 2>&1; then
    dig +time=2 +tries=1 +short "$1" A 2>/dev/null \
      | grep -E '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$' | head -1
  elif command -v host >/dev/null 2>&1; then
    host -W 2 "$1" 2>/dev/null \
      | awk '/has address/ {print $4; exit}'
  else
    getent ahostsv4 "$1" 2>/dev/null | awk '{print $1; exit}'
  fi
}
for sub in auth billing s3 s3-console; do
  fqdn="${sub}.${APP_DOMAIN}"
  ip="$(resolve "$fqdn")"
  if [[ -n "$ip" ]]; then pass "$fqdn → $ip"; else fail "$fqdn has no A record"; fi
done

# ── TLS + HTTP probes ───────────────────────────────────────────────────────
section "Public HTTPS probes"
probe() {
  local url="$1" expect_pattern="$2" label="$3"
  local code
  code="$(curl -sS -o /tmp/_probe_body -w '%{http_code}' --max-time 10 "$url" 2>/dev/null || echo "000")"
  local body_head
  body_head="$(head -c 80 /tmp/_probe_body 2>/dev/null | tr '\n' ' ')"
  if [[ "$code" =~ $expect_pattern ]]; then
    pass "$label  [HTTP $code]"
  else
    fail "$label  [HTTP $code] body: ${body_head}"
  fi
}

# Zitadel: OIDC discovery is canonical liveness signal
probe "https://auth.${APP_DOMAIN}/.well-known/openid-configuration" '^200$' "zitadel /.well-known/openid-configuration"
# Lago: front returns 200 (Vite app) or 302 to login
probe "https://billing.${APP_DOMAIN}/health"                          '^(200|301|302)$' "lago /health"
# RustFS: minio-compatible liveness
probe "https://s3.${APP_DOMAIN}/minio/health/live"                    '^(200|204)$' "rustfs S3 /minio/health/live"
# RustFS console (may require auth → 200 or 401)
probe "https://s3-console.${APP_DOMAIN}/"                             '^(200|301|302|401)$' "rustfs console /"

# Detect Traefik default 404 (means no router matched)
section "Router presence (Traefik default-404 detection)"
for sub in auth billing s3 s3-console; do
  fqdn="${sub}.${APP_DOMAIN}"
  body="$(curl -sS --max-time 10 "https://${fqdn}/" 2>/dev/null || true)"
  if [[ "$body" == "404 page not found" ]]; then
    fail "${fqdn} → Traefik default 404 (no router matches this Host header)"
  else
    pass "${fqdn} → routed to an application"
  fi
done

# ── Optional host inspection over SSH ───────────────────────────────────────
if [[ -n "$SSH_HOST" ]]; then
  section "Host inspection via ssh ${SSH_HOST}"

  run() { ssh -o BatchMode=yes -o ConnectTimeout=5 "$SSH_HOST" "$@"; }

  if ! run true 2>/dev/null; then
    fail "cannot ssh to $SSH_HOST (need passwordless key auth)"
  else
    pass "ssh reachable"

    # Container health
    section "Container status (prefix: ${STACK_PREFIX})"
    statuses="$(run "docker ps -a --format '{{.Names}}\t{{.Status}}' | grep '^${STACK_PREFIX}-' || true")"
    if [[ -z "$statuses" ]]; then
      fail "no containers found with prefix ${STACK_PREFIX}-"
    else
      while IFS=$'\t' read -r name status; do
        case "$status" in
          *"(healthy)"*)         pass "$name — $status" ;;
          *"Up"*"(health: starting)"*) warn "$name — $status" ;;
          *"Up"*)                pass "$name — $status" ;;
          *"Exited (0)"*)        pass "$name — $status (one-shot init OK)" ;;
          *"Restarting"*)        fail "$name — $status" ;;
          *)                     fail "$name — $status" ;;
        esac
      done <<< "$statuses"
    fi

    # Traefik label interpolation check (the #1 cause of Traefik 404)
    section "Traefik label interpolation"
    z_container="$(run "docker ps -a --format '{{.Names}}' | grep '^${STACK_PREFIX}-.*-zitadel-1$' | head -1" 2>/dev/null)"
    if [[ -z "$z_container" ]]; then
      fail "zitadel container not found — cannot verify labels"
    else
      label="$(run "docker inspect '$z_container' --format '{{ index .Config.Labels \"traefik.http.routers.zitadel.rule\" }}'")"
      info "zitadel router rule label: $label"
      if [[ "$label" == "Host(\`auth.${APP_DOMAIN}\`)" ]]; then
        pass "label fully interpolated"
      elif [[ -z "$label" ]]; then
        fail "label MISSING — compose labels block not applied"
      else
        fail "label NOT interpolated correctly (likely \${APP_DOMAIN} was not visible to compose at deploy time)"
      fi
    fi

    # Network attachment
    section "Network attachment"
    for c in zitadel lago-api rustfs; do
      cname="$(run "docker ps -a --format '{{.Names}}' | grep '^${STACK_PREFIX}-.*-${c}-1$' | head -1" 2>/dev/null)"
      [[ -z "$cname" ]] && { fail "$c: container not found"; continue; }
      nets="$(run "docker inspect '$cname' --format '{{range \$k, \$v := .NetworkSettings.Networks}}{{\$k}} {{end}}'")"
      if [[ "$nets" == *"dokploy-network"* ]]; then
        pass "$c attached to dokploy-network (nets: $nets)"
      else
        fail "$c NOT on dokploy-network (nets: $nets) — Traefik cannot reach it"
      fi
    done

    # Traefik can see the routers
    section "Traefik runtime view"
    if run "docker ps --format '{{.Names}}' | grep -q '^dokploy-traefik'"; then
      routers="$(run "docker exec dokploy-traefik wget -qO- http://localhost:8080/api/http/routers 2>/dev/null | grep -oE '\"name\":\"[^\"]+\"' | sort -u")"
      if [[ -z "$routers" ]]; then
        warn "Traefik API responded with no routers (or API not enabled)"
      else
        for want in zitadel lago rustfs-s3 rustfs-console; do
          if grep -q "\"name\":\"${want}@docker\"" <<< "$routers"; then
            pass "Traefik router present: ${want}@docker"
          else
            fail "Traefik router MISSING: ${want}@docker"
          fi
        done
      fi
    else
      warn "no container named dokploy-traefik — skipping Traefik API check"
    fi
  fi
fi

section "Summary"
if [[ $FAIL -eq 0 ]]; then
  printf '  %sAll checks passed%s\n\n' "$GREEN" "$RESET"
  exit 0
else
  printf '  %s%d check(s) failed%s\n\n' "$RED" "$FAIL" "$RESET"
  exit 1
fi
