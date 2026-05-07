-- Source-of-truth table for bulk-loaded capability specs.
-- Each row defines a single capability that can be generated at boot or via hot-reload.
-- Domain partitioning is handled by `namespace` (e.g. erp.po, crm.lead, accounting.gl).
--
-- strategy: 'wasm' | 'prompt' | 'native' | 'dynamic_prompt'
-- payload:  strategy-specific config JSON (e.g. { "wasm_hash": "...", "prompt_id": 1 })

CREATE TABLE IF NOT EXISTS capability_specs (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    namespace     TEXT NOT NULL,
    tool_name     TEXT NOT NULL,
    description   TEXT NOT NULL,
    input_schema  JSONB NOT NULL,
    output_schema JSONB,
    strategy      TEXT NOT NULL,
    payload       JSONB NOT NULL DEFAULT '{}',
    tags          TEXT[] NOT NULL DEFAULT '{}',
    enabled       BOOL NOT NULL DEFAULT true,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (namespace, tool_name)
);

CREATE INDEX IF NOT EXISTS capability_specs_ns_idx ON capability_specs (namespace);
CREATE INDEX IF NOT EXISTS capability_specs_tags_idx ON capability_specs USING gin (tags);
CREATE INDEX IF NOT EXISTS capability_specs_enabled_idx ON capability_specs (enabled);

-- Trigger: notify hot-reload listeners on any change.
CREATE OR REPLACE FUNCTION notify_capability_specs_changed() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('capability_specs_changed',
        json_build_object(
            'namespace', COALESCE(NEW.namespace, OLD.namespace),
            'tool_name', COALESCE(NEW.tool_name, OLD.tool_name),
            'op', TG_OP
        )::text
    );
    RETURN COALESCE(NEW, OLD);
END $$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS capability_specs_changed_trg ON capability_specs;
CREATE TRIGGER capability_specs_changed_trg
    AFTER INSERT OR UPDATE OR DELETE ON capability_specs
    FOR EACH ROW EXECUTE FUNCTION notify_capability_specs_changed();
