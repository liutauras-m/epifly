**✅ Excellent Question.**

Here’s how the **ConusAI Platform** should (and can) be **multitenant** according to **2026 Rig best practices**.

### What “Multitenant” Means in This Context

- Multiple users / teams / organizations (tenants) can use the same deployed instance.
- Strong **isolation** of data, files, prompts, capabilities, and billing.
- Still share the same agent runtime and capabilities pool for efficiency.

### Recommended Multitenancy Design (Rig-Aligned)

| Layer                    | Implementation (Best Practice 2026)                                                                 | Rig Alignment |
|--------------------------|-------------------------------------------------------------------------------------------------------|---------------|
| **Tenant Identification** | `X-Tenant-ID` header or JWT claim (`tenant_id`)                                                      | Full |
| **Authentication**       | JWT + OAuth2 (Clerk, Auth0, Supabase, or custom) + `tenant_id` claim                                 | Full |
| **Data Isolation**       | Every file path prefixed with `tenants/{tenant_id}/` + path safety checks                            | Strong |
| **Storage**              | MinIO buckets per tenant (`conusai-tenant-{tenant_id}`) or single bucket with prefix isolation       | Strong |
| **Workspace**            | Each tenant has its own workspace root (`/workspaces/{tenant_id}`)                                   | Full |
| **Capabilities**         | Shared capabilities (OCR, invoice, etc.) but with tenant-scoped auth (e.g. Google OAuth per tenant) | Excellent |
| **Agent Context**        | `TenantContext` injected into every agent run                                                        | Rig-native |
| **Vector Store**         | Qdrant collections per tenant (`capabilities_{tenant_id}`, `memories_{tenant_id}`)                   | Recommended |
| **Rate Limiting**        | Per-tenant rate limiting in `agent-gateway` middleware                                               | Good |
| **Evals & Logging**      | All logs and evals tagged with `tenant_id`                                                           | Full observability |

### Core Implementation Changes Needed

#### 1. `TenantContext` in `agent-core`

```rust
// crates/agent-core/src/context/tenant.rs
#[derive(Clone)]
pub struct TenantContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub plan: PlanTier,           // free, pro, enterprise
    pub workspace_root: PathBuf,
}
```

#### 2. Updated `GeneralAgentBuilder`

```rust
pub fn build_for_tenant(
    config: AgentRuntimeConfig,
    tenant: TenantContext,
) -> GeneralAgent {
    GeneralAgentBuilder::new()
        .with_tenant_context(tenant)
        .with_capability_registry(registry.with_tenant(&tenant))
        .build()
}
```

#### 3. Path Safety + Storage Isolation (Critical)

In `common::path_safety.rs`:

```rust
pub fn join_under_tenant(root: &Path, tenant_id: &str, rel: &str) -> Result<PathBuf> {
    let safe_path = root.join("tenants").join(tenant_id).join(rel);
    // strict validation
    Ok(safe_path)
}
```

#### 4. Capability Isolation Strategy

- **Shared capabilities** (OCR, VideoTS, invoice-processing) → run in multi-tenant mode with tenant prefixing.
- **Tenant-specific capabilities** (e.g. private Google Workspace) → scoped OAuth tokens stored per tenant.
- `file-storage` capability automatically prefixes all uploads with tenant.

#### 5. Qdrant Multitenancy

```rust
// Different collection per tenant
let collection = format!("capabilities_{}", tenant_id);
```

### Rig Best Practices Alignment (2026)

Rig itself is **lightweight and unopinionated** about multitenancy.  
The recommended pattern in the Rig community is:

- **Context Injection** — Pass tenant context into the agent builder.
- **Scoped Tools** — Capabilities can return tenant-filtered tools.
- **Vector Store Namespacing** — Use tenant-prefixed collections/indexes.
- **Middleware Layer** — Handle tenant resolution in the gateway.

Your platform already has a very good foundation for this.

