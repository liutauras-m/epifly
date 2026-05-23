# ADR-0007 — Capabilities as Everything

**Status:** Accepted  
**Date:** 2026-05-20  
**Author:** Engineering

---

## Context

The platform originally had hard-coded domain chains (`InvoicePipeline`, `OcrPipeline`, etc.) compiled into `agent-core`. Adding a new extraction domain required Rust code changes in a shared crate, a new gateway registration block in `main.rs`, and no discoverability surface for the capability.

The goals of this ADR are:

1. Every domain concern — extraction, storage, conversion, orchestration — is expressed as a `CapabilityProvider` declared by a TOML manifest.
2. No domain-specific types survive in core crates (`agent-core`, `common`).
3. New domains are added by dropping a `capability.toml` directory and implementing (or reusing) a factory.
4. The agent loop uses the `SemanticCapabilityRouter` as the sole source of tools — no domain branching.

---

## Decision

### Manifest v2 schema

Every capability is declared by a `capability.toml` file under `apps/backend/capabilities/<name>/`. The `schema_version = "2.0"` manifest adds:

| Field | Purpose |
|---|---|
| `namespace` | Dot-separated taxonomy path (e.g. `extract.fields.invoice`) |
| `category` | Root taxonomy segment (validates against allowed set) |
| `accepts` | MIME globs the capability can process (e.g. `["application/pdf", "image/*"]`) |
| `emits` | MIME types produced |
| `idempotent` | Whether repeated invocation with same input is safe |
| `cost_hint` | Relative cost band (`low`/`medium`/`high`) |
| `requires` | Other capability names that must be available |

### Taxonomy

Eight root categories cover all capability kinds:

```
storage   — read/write workspace, object store
compute   — pure computation (WASM, code execution)
sense     — detection/classification
extract   — field or text extraction from files
convert   — format conversion
compose   — content generation
deliver   — external integrations (email, Slack, APIs)
plan      — orchestration primitives
```

### Factory dispatch

`CapabilityRegistry::with_default_factories()` registers:

- `McpFactory` — for `kind = "mcp"` (MCP protocol server tools)
- `WasmFactory` — for `kind = "wasm"` (WebAssembly modules)
- `ChainFactory` — for `kind = "chain"` (LLM prompt chains)

`NativeStorageFactory` is registered separately in `state.rs` because it captures `Arc<dyn WorkspaceStore>` and `Arc<dyn WorkspaceContentStore>`. It handles `kind = "native"` capabilities and dispatches on `config.op`:

| `op` | Provider |
|---|---|
| `workspace` | `WorkspaceNativeProvider` (save_document, list_folders) |
| `read_text` | `ReadTextProvider` |
| `write_text` | `WriteTextProvider` |

Job-backed capabilities (e.g. `transcribe-video`) are registered programmatically in `state.rs` using `registry.register_provider()` because they need `Arc<JobExecutor>` from the `jobs` crate (not available to `agent-core`).

### Hot-reload

`ManifestWatcher` uses the `notify` crate (v7) to watch the capabilities directory with a 250 ms debounce. On `capability.toml` change it calls `CapabilityRegistry::reload_capability()` which re-reads the manifest and recreates the provider via the registered factory.

### Semantic routing

`SemanticCapabilityRouter` uses ANN vector search (Qdrant) to pre-select tools from the catalog, then applies `AttachmentHint` MIME post-filtering. The router replaces all domain branching in the agent loop.

---

## Consequences

**Positive:**
- New extraction domain = 1 TOML file + optional provider (often reusing `chain` kind).
- `agent-core` has no domain knowledge.
- All capabilities discoverable via `GET /v1/capabilities`.
- Capability metadata (cost, idempotency, accepts/emits) is machine-readable for planning.

**Negative:**
- `NativeStorageFactory` and job-backed capabilities require gateway-level wiring; they cannot be loaded by `CapabilityDiscovery` alone.
- The `CapabilityFactory::supports()` method only receives `kind` and `name` — it cannot inspect manifest config. This means the factory dispatch is name/kind based, not config based.

---

## Alternatives Rejected

- **Keeping domain chains**: Would require Rust changes for every new domain; no discoverability.
- **Single omnibus `NativeFactory` in `agent-core`**: Would require `agent-core` to depend on `jobs`, creating a circular dependency.
- **Adding `can_handle(card: &CapabilityCard)` to `CapabilityFactory`**: Too large a breaking change at this stage.


---

## Addendum — 2026-05-22 (capabilities-consolidation refactor)

**Capabilities are *domain-level*.** One capability ≡ one coherent toolkit;
granularity lives in `[[tools]]`, not in directories.

The 15 granular `storage.*` capabilities collapsed into two:
`storage-workspace` (11 workspace-node tools) and `storage-fs` (5 filesystem-path tools).
Provider dispatch is on `tool_name` in `invoke()` — the canonical pattern for multi-tool
native capabilities. `ToolManifest` schema is unchanged; no `[[config.tools]]` extension exists.

**Rule**: ANN embeddings index one card per domain. If two capabilities compete for the
same prompt, they are the same domain and should be merged. Splitting a domain across
cards is an anti-pattern.
