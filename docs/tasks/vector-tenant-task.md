5. Vector-store tenant isolation needs paranoia

The doc says Qdrant stores capability embeddings and content embeddings, with tenant filtering on content vectors. This is acceptable only if tenant filtering is mandatory, tested, and impossible to bypass.

Security people increasingly criticize naive centralized RAG/vector architectures because they often bypass original access controls and create new leakage surfaces.

Required improvement:

Add a TenantScopedVectorStore wrapper so code cannot query content vectors without tenant context.

Bad:

vector_store.search(query, filter)

Better:

tenant_vector_store.for_tenant(tenant_id).search(query)

And add tests:

tenant_a document indexed
tenant_b query semantically matches document
expected: zero tenant_a results

Do not trust developer discipline here. Developers are just users with commit access.