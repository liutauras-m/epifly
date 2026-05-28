-- Phase 6: Tenant identity binding table
-- Maps (zitadel_issuer, zitadel_org_id) → application tenant_id.
-- User identity = (issuer, sub) — never email.

CREATE TABLE IF NOT EXISTS tenant_identity_bindings (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id       text NOT NULL UNIQUE,
  zitadel_issuer  text NOT NULL,
  zitadel_org_id  text NOT NULL,
  plan_tier       text NOT NULL DEFAULT 'free',
  status          text NOT NULL DEFAULT 'active',
  created_by_sub  text NOT NULL,
  created_at      timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT uq_issuer_org UNIQUE (zitadel_issuer, zitadel_org_id)
);
