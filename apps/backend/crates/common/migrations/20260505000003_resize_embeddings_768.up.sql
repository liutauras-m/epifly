-- Resize embedding columns from 1536 (OpenAI) to 768 (local fastembed default).
-- Must drop the diskann indexes before altering column type.

DROP INDEX IF EXISTS cap_embed_idx;
ALTER TABLE capability_embeddings ALTER COLUMN embedding TYPE vector(768);
CREATE INDEX cap_embed_idx ON capability_embeddings USING diskann (embedding);

DROP INDEX IF EXISTS content_embed_idx;
ALTER TABLE content_embeddings ALTER COLUMN embedding TYPE vector(768);
CREATE INDEX content_embed_idx ON content_embeddings USING diskann (embedding);
