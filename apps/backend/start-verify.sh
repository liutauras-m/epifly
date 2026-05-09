#!/usr/bin/env bash
set -a
source /Users/liutauras.m/Projects/conusai-platform/.env.local
set +a

export SUPER_ADMIN_EMAILS="Super Admin"
export CONUSAI_SERVER__PORT=8088
export CONUSAI_TEST_MODE=1

cd /Users/liutauras.m/Projects/conusai-platform/apps/backend
exec cargo run -p agent-gateway
