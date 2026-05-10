ALTER TABLE capability_specs
    ADD COLUMN IF NOT EXISTS tenant_scope TEXT[] NOT NULL DEFAULT '{}';

CREATE INDEX IF NOT EXISTS capability_specs_scope_idx
    ON capability_specs USING gin (tenant_scope);
