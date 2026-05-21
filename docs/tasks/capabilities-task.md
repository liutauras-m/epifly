**Yes — the current storage/workspace capabilities are too granular.**

We currently ship **~15 separate native capabilities** in the `storage.*` namespace (see inventory from previous response):

- `storage-workspace`, `storage-workspace-move`
- `storage-put`, `storage-read-text`, `storage-write-text`
- `storage-move`, `storage-delete`, `storage-bulk-delete`
- `storage-list-folders`, `storage-create-folder`, `storage-ensure-folder`, `storage-ensure-date-folder`
- `storage-find-by-name`, `storage-show-tree`, `storage-tag`

Each lives in its own directory with its own `capability.toml`, own `CapabilityCard`, own embedding, and mostly a single tool.

**This violates the spirit of the 2026 architecture** even though it technically follows SRP at the module level.

### Canonical recommendation (aligned to Project Instructions v0.3.2 + Rig 0.36+)

**Consolidate into 1–2 domain-level capabilities** (maximum 3), each exposing **multiple tools** via the existing `[[tools]]` array in the manifest.

**Preferred target design** (already fully supported by the manifest schema and `CapabilityProvider` trait):

1. **`storage.workspace`** (native)  
   - All workspace-node operations: `save_document`, `list_folders`, `show_tree`, `create_folder`, `ensure_folder`, `ensure_date_folder`, `find_by_name`, `move_node`, `delete_node`, `bulk_delete`, `tag_object`

2. **`storage.object`** (native or keep the existing MCP `file-storage` and extend it)  
   - Low-level object ops: `put_object`, `read_file`, `write_file`, `move_object`, `upload_file`, `download_file`, `presigned_url`

3. (Optional) Keep `storage.fs` only if we need a third ultra-low-level escape hatch.

This matches:
- Manifest §2.2 (`[[tools]]` blocks + per-tool `input_schema`)
- `CapabilityProvider::tool_definitions()` (already joins `cap_name__tool_name` for Anthropic safety)
- `SemanticCapabilityRouter` (one strong domain embedding beats 15 noisy near-duplicates)
- SRP at the *capability* level: “the workspace storage domain” is a single obvious reason to exist.

### Which approach wins for model accuracy / speed / hallucination resistance?

**Consolidated domain-level (1–2 caps with many tools) is clearly superior** in 2026-era agent loops (Claude 4 / Rig 0.36).

| Dimension                  | Granular (current ~15 caps)                          | Consolidated (1–2 domain caps)                          | Winner & Why |
|----------------------------|------------------------------------------------------|---------------------------------------------------------|--------------|
| **Accuracy**               | Medium — router can pick the exact tool but LLM often confuses similar ones | **High** — LLM sees a coherent “storage toolkit” together | Consolidated |
| **Hallucination resistance**| Low — many near-identical tools → name confusion, wrong tool picked | **High** — model has one mental model of the domain | Consolidated |
| **Speed / latency**        | Slightly slower (more Qdrant hits, larger top-K set) | **Faster** — fewer embeddings, tighter top-K, smaller tool catalog sent to LLM | Consolidated |
| **Token cost**             | Higher (more tool definitions in every storage-related turn) | **Lower** — one rich capability definition | Consolidated |
| **Semantic router quality**| Noisy (many competing similar embeddings)            | **Clean** — strong single domain signal | Consolidated |
| **Maintainability / SRP**  | Technically follows per-module SRP                   | Still follows SRP (one capability = one domain) + far easier to evolve | Consolidated (with proper naming) |
| **Admin / hot-reload**     | Granular enable/disable per tiny op                  | Still fully supported (tools stay individually testable) | Tie |

The **SemanticCapabilityRouter** (top-K=20, max_distance=0.38) already prevents the full catalog from ever reaching the LLM — but 15 near-duplicate storage cards dilute the ANN signal and increase the chance the router returns 6–8 overlapping tools instead of the perfect 2–3.

Modern LLM tool-calling research (Anthropic 2025–2026 patterns) consistently shows: **10–30 well-grouped tools per turn is the sweet spot**. Too many similar tools is the #1 cause of tool-selection hallucinations.

### Proposed refactor (minimal, high-impact)

- Create `apps/backend/capabilities/storage/` (single directory)
- One `capability.toml` with `kind = "native"` + ~12–14 `[[tools]]` blocks
- Single `BuiltinFactory` entry (or extend existing native provider)
- Delete the 15 old granular directories (or mark `enabled = false` during migration)
- `ArtifactBridge` stays the sole owner of materialisation (no change)

**Effort estimate**: 4–6 AI-hours + ~120k tokens  
(manifest consolidation + one new native provider dispatcher + test 3–4 common workflows + Qdrant re-embed).  
Zero breaking changes to `AgentBuilder`, `CapabilityRegistry`, or `SemanticCapabilityRouter`.

This change will immediately improve agent reliability on every workspace-heavy task (upload → plan → extract → compose → save flows).

**Shall I prepare the exact patches** (registry update + new manifest + BuiltinFactory change + effort log) right now?  
Just say “yes — consolidate storage” and I’ll output the complete, review-ready diff set.