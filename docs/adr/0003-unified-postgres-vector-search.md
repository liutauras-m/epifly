# ADR 0003 — Unified Postgres Vector Search (replaces Qdrant)

**Status:** Superseded by [ADR 0009](0009-redb-qdrant-rustfs.md)  
**Date:** 2026-05-05  
**Deciders:** Platform team

---

## Context

The original plan referenced Qdrant as a dedicated vector database for semantic search
over capabilities and workspace content. Qdrant requires an additional service, a separate
deployment, and a separate client library. Meanwhile the project already runs Postgres with
the `pgvector` and `pgvectorscale` (diskann) extensions for relational data.

Two distinct search problems exist:

1. **Capability ANN search** — find the top-k tool cards whose description is closest to a
   user's query embedding.
2. **Workspace semantic search** — find workspace nodes (conversations, files) whose indexed
   text content is closest to a user's query.

---

## Decision

Replace Qdrant with **Postgres + pgvector** for all vector operations:

| Concern | Solution |
|---------|----------|
| Embedding model | OpenAI `text-embedding-3-small` (1536 dims) via direct HTTP |
| Vector columns | `VECTOR(1536)` in `capability_embeddings` and `content_embeddings` |
| ANN index | `USING diskann` (pgvectorscale) for sub-linear search |
| Distance metric | Cosine (`<=>`) via raw `sqlx::query()` with `$1::vector` cast |
| rig-postgres | Used for `PgVectorDistanceFunction` / `PgSearchFilter` type imports |
| Incremental indexing | Custom `WorkspaceIndexer` (replaces CocoIndex Python library) |

`rig-postgres 0.2.5` is included as a dependency because its schema (`id UUID, document JSONB`)
does not match our tables — raw `sqlx::query()` calls execute the actual queries while
`rig-postgres` types satisfy the trait boundary at the call site.

---

## Consequences

**Good:**
- One fewer service to deploy and operate.
- Transactional consistency between relational data and vector data.
- No separate Qdrant client SDK; standard sqlx connection pool reused.
- SHA-256 content hash prevents redundant re-embedding on unchanged files.

**Bad / tradeoffs:**
- Maximum scale bounded by Postgres I/O, not a purpose-built ANN engine.
- diskann index is only available with TimescaleDB / pgvectorscale; plain pgvector
  falls back to the slower `ivfflat` or exact scan.
- No Qdrant → `vector search disabled` if `OPENAI_API_KEY` is absent (falls back to
  `NoopEmbeddingService`; full-text search still works).

---

## Alternatives Considered

| Option | Rejected because |
|--------|-----------------|
| Keep Qdrant | Extra service, extra credentials, diverges from "one Postgres" principle |
| CocoIndex (Rust) | Does not exist as a Rust crate on crates.io; only a Python SDK |
| fastembed | Not present in workspace deps; local model adds RAM pressure |
| rig-postgres native queries | Incompatible schema (`id UUID`, `document JSONB`); cannot be used directly |

---

## Implementation

- `agent-core/src/indexing/embedding_service.rs` — `EmbeddingService` trait + `OpenAiEmbeddingService` + `NoopEmbeddingService`
- `agent-core/src/indexing/coco_indexer.rs` — `WorkspaceIndexer` (walk, chunk, embed, upsert)
- `agent-core/src/indexing/real_fs_watcher.rs` — polling watcher wrapping `WorkspaceIndexer`
- `agent-core/src/vector_store/postgres.rs` — `PgVectorStore` with raw sqlx queries
- `agent-gateway/src/routes/search.rs` — capability ANN search with hash-based refresh
- `agent-gateway/src/routes/workspaces.rs` — `?mode=semantic` routes to `semantic_search_nodes`
- `agent-gateway/src/main.rs` — wires `WorkspaceIndexer` + `RealFsWatcher` when `WORKSPACES_ROOT` is set
