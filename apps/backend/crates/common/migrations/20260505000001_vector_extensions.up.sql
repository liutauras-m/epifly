-- Enable pgvector and pgvectorscale extensions.
-- Must run as superuser (the default POSTGRES_USER=conusai is superuser in the
-- Docker dev environment; for production use a pre-provisioned extension step).
CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS vectorscale CASCADE;

-- Create diskann indexes now that the extension is available.
CREATE INDEX IF NOT EXISTS cap_embed_idx
    ON capability_embeddings USING diskann (embedding);

CREATE INDEX IF NOT EXISTS content_embed_idx
    ON content_embeddings USING diskann (embedding);
