-- Threads
CREATE TABLE IF NOT EXISTS threads (
    id            TEXT PRIMARY KEY,
    tenant_id     TEXT NOT NULL,
    title         TEXT,
    summary       TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_active   TIMESTAMPTZ NOT NULL DEFAULT now(),
    message_count INT NOT NULL DEFAULT 0,
    metadata      JSONB NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS threads_tenant_idx ON threads(tenant_id);

-- Messages
CREATE TABLE IF NOT EXISTS messages (
    id          BIGSERIAL PRIMARY KEY,
    thread_id   TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    seq         INT NOT NULL,
    role        TEXT NOT NULL CHECK (role IN ('user','assistant','tool')),
    content     TEXT NOT NULL,
    tool_calls  JSONB NOT NULL DEFAULT '[]',
    timestamp   TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(thread_id, seq)
);
CREATE INDEX IF NOT EXISTS messages_thread_seq_idx ON messages(thread_id, seq);

-- Workspace nodes
CREATE TABLE IF NOT EXISTS workspace_nodes (
    id            TEXT PRIMARY KEY,
    tenant_id     TEXT NOT NULL,
    owner_id      TEXT NOT NULL,
    parent_id     TEXT REFERENCES workspace_nodes(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL CHECK (kind IN ('folder','conversation','file')),
    name          TEXT NOT NULL,
    virtual_path  TEXT NOT NULL,
    last_modified TIMESTAMPTZ NOT NULL DEFAULT now(),
    shared_with   TEXT[] NOT NULL DEFAULT '{}',
    metadata      JSONB NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS ws_tenant_idx ON workspace_nodes(tenant_id);
CREATE INDEX IF NOT EXISTS ws_parent_idx ON workspace_nodes(parent_id);
CREATE INDEX IF NOT EXISTS ws_path_idx   ON workspace_nodes(virtual_path);

-- Audit log
CREATE TABLE IF NOT EXISTS audit_events (
    id          TEXT PRIMARY KEY,
    tenant_id   TEXT NOT NULL,
    timestamp   TIMESTAMPTZ NOT NULL DEFAULT now(),
    action      TEXT NOT NULL,
    tool        TEXT,
    status      TEXT NOT NULL,
    duration_ms INT,
    metadata    JSONB NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS audit_tenant_ts_idx ON audit_events(tenant_id, timestamp DESC);

-- Capability embeddings
CREATE TABLE IF NOT EXISTS capability_embeddings (
    capability_id TEXT PRIMARY KEY,
    content       TEXT NOT NULL,
    embedding     vector(768),
    metadata      JSONB NOT NULL DEFAULT '{}',
    namespace     TEXT NOT NULL DEFAULT '',
    tags          TEXT[] NOT NULL DEFAULT '{}',
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS cap_embed_idx
    ON capability_embeddings USING diskann (embedding);
CREATE INDEX IF NOT EXISTS cap_embed_ns_idx
    ON capability_embeddings (namespace);
CREATE INDEX IF NOT EXISTS cap_embed_tags_idx
    ON capability_embeddings USING gin (tags);

-- Workspace content embeddings (for semantic search)
CREATE TABLE IF NOT EXISTS content_embeddings (
    id          TEXT PRIMARY KEY,
    node_id     TEXT NOT NULL REFERENCES workspace_nodes(id) ON DELETE CASCADE,
    chunk_index INT NOT NULL DEFAULT 0,
    content     TEXT NOT NULL,
    embedding   vector(768),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS content_embed_idx
    ON content_embeddings USING diskann (embedding);

-- DB-backed versioned prompt capabilities
CREATE TABLE IF NOT EXISTS dynamic_prompts (
    capability_name TEXT NOT NULL,
    version         INT  NOT NULL DEFAULT 1,
    system_prompt   TEXT,
    user_template   TEXT NOT NULL,
    few_shot        JSONB NOT NULL DEFAULT '[]',
    output_schema   JSONB,
    model           TEXT NOT NULL,
    max_tokens      INT  NOT NULL DEFAULT 1024,
    vision          BOOL NOT NULL DEFAULT false,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (capability_name, version)
);
CREATE INDEX IF NOT EXISTS dyn_prompts_latest_idx
    ON dynamic_prompts (capability_name, version DESC);

-- Capability specs (bulk source of truth, domain-neutral; partition by namespace)
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
    -- tenant_scope: empty = global; non-empty = only these tenant IDs can see this capability
    tenant_scope  TEXT[] NOT NULL DEFAULT '{}',
    enabled       BOOL NOT NULL DEFAULT true,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (namespace, tool_name)
);
CREATE INDEX IF NOT EXISTS capability_specs_ns_idx    ON capability_specs (namespace);
CREATE INDEX IF NOT EXISTS capability_specs_tags_idx  ON capability_specs USING gin (tags);
CREATE INDEX IF NOT EXISTS capability_specs_scope_idx ON capability_specs USING gin (tenant_scope);

-- Async indexing queue for tool-output artifacts
CREATE TABLE IF NOT EXISTS indexing_queue (
    id           BIGSERIAL PRIMARY KEY,
    node_id      TEXT NOT NULL,
    object_key   TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'pending',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,
    error        TEXT,
    UNIQUE (node_id)
);
CREATE INDEX IF NOT EXISTS indexing_queue_status_idx ON indexing_queue (status, created_at);

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
