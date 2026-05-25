#!/bin/sh
# Create the per-service databases Zitadel + Lago expect.
# Runs once on first cluster init (Postgres ignores it on subsequent boots).
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
  SELECT 'CREATE DATABASE zitadel'
   WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'zitadel')\gexec
  SELECT 'CREATE DATABASE lago'
   WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'lago')\gexec
EOSQL
