-- Add namespace + tags columns to capability_embeddings for semantic routing.
-- namespace: dot-separated slug, e.g. 'accounting.invoice'
-- tags: secondary classification axes for tag-any filtering

ALTER TABLE capability_embeddings
    ADD COLUMN IF NOT EXISTS namespace TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS tags      TEXT[] NOT NULL DEFAULT '{}';

CREATE INDEX IF NOT EXISTS cap_embed_ns_idx
    ON capability_embeddings (namespace);

CREATE INDEX IF NOT EXISTS cap_embed_tags_idx
    ON capability_embeddings USING gin (tags);
