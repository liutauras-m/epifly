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
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS cap_embed_idx
    ON capability_embeddings USING diskann (embedding);

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
