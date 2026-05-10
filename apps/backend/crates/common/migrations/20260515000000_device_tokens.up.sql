CREATE TABLE device_tokens (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id    TEXT        NOT NULL,
    device_label TEXT        NOT NULL,
    token_hash   BYTEA       NOT NULL UNIQUE,  -- blake3(plaintext_token)
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen    TIMESTAMPTZ,
    revoked_at   TIMESTAMPTZ
);

CREATE INDEX device_tokens_tenant_idx
    ON device_tokens (tenant_id)
    WHERE revoked_at IS NULL;
