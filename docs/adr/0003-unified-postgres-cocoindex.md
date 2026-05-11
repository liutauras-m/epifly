# ADR 0003 - Unified Postgres + CocoIndex (replaces Qdrant)

Status: Superseded by [ADR 0009](0009-redb-qdrant-rustfs.md)  
Date: 2026-05-05  
Deciders: Platform team

## Context

The original design used Qdrant for vector retrieval, but the platform already depends on Postgres and now runs pgvector + pgvectorscale. Operating both Postgres and Qdrant increased deployment complexity, duplicated data boundaries, and introduced extra failure modes.

Two search domains had to be supported:

1. Capability semantic search over capability card content.
2. Workspace semantic search over indexed file and node content.

## Decision

Use Postgres as the single persistence and vector retrieval backend:

- Embeddings: OpenAI `text-embedding-3-small` (1536 dimensions).
- Storage: `capability_embeddings` and `content_embeddings` with `vector(1536)` columns.
- Retrieval: ANN with cosine distance using pgvector operators in Postgres SQL.
- Indexing: CocoIndex-style incremental indexer implemented by `WorkspaceIndexer` + `RealFsWatcher`.
- Adapter strategy: `rig-postgres` types are used for distance/filter configuration, while raw `sqlx` queries are used for our custom table layouts.

## Rationale

- Single data platform is operationally simpler than Postgres + Qdrant.
- ANN queries and relational joins are colocated in one datastore.
- Existing sqlx pool, migrations, and observability remain consistent.
- Incremental indexing with content hashing avoids repeated embedding of unchanged data.

## Tradeoffs

Pros:
- Fewer moving parts and credentials.
- Unified backup/restore and runtime diagnostics.
- Stronger consistency between metadata rows and embedding rows.

Cons:
- Search scale is bounded by Postgres resources.
- pgvectorscale/diskann features depend on runtime image/extensions.
- If embedding provider is unavailable, semantic path degrades and fallback paths are required.

## Alternatives Considered

- Keep Qdrant: rejected due to extra service and duplicated operational surface.
- Rust CocoIndex crate: no production-ready upstream crate available; custom indexer implemented instead.
- Full rig-postgres native schema: incompatible with existing table design; adapter kept narrow and explicit.

## Implementation Scope

- Capability ANN search route migrated to vector-first retrieval.
- Workspace semantic route migrated to embedding + ANN retrieval.
- `index_content` writes chunked content embeddings.
- Gateway startup wires `WORKSPACES_ROOT` indexer watcher.
- Test mode avoids requiring a live Postgres connection for job execution paths.
