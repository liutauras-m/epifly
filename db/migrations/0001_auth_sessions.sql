-- Phase 1: Auth session store
-- Run once per environment. Idempotent (IF NOT EXISTS).

CREATE TABLE IF NOT EXISTS auth_sessions (
  id                text PRIMARY KEY,
  user_iss          text NOT NULL,
  user_sub          text NOT NULL,
  tenant_org_id     text NOT NULL,
  access_ct         bytea NOT NULL,
  refresh_ct        bytea NOT NULL,
  id_token_ct       bytea,
  access_expires_at timestamptz NOT NULL,
  created_at        timestamptz NOT NULL DEFAULT now(),
  last_seen_at      timestamptz NOT NULL DEFAULT now(),
  revoked_at        timestamptz
);

CREATE INDEX IF NOT EXISTS auth_sessions_user
  ON auth_sessions(user_iss, user_sub);

CREATE TABLE IF NOT EXISTS auth_oidc_transactions (
  state         text PRIMARY KEY,
  code_verifier text NOT NULL,
  nonce         text NOT NULL,
  return_to     text NOT NULL,
  created_at    timestamptz NOT NULL DEFAULT now(),
  consumed_at   timestamptz
);
