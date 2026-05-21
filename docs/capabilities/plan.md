# Capabilities Consolidation Refactor — Detailed Plan

> **Goal.** (1) Collapse the 15 granular `storage.*` capabilities into **two
> domain-level capabilities** (`storage-workspace`, `storage-fs`) without
> changing tool semantics, breaking the agent loop, or touching
> `CapabilityRegistry` / `SemanticCapabilityRouter` / `ToolExecutor`.
> (2) Introduce a new **`code-project`** capability so the platform can
> scaffold and edit code projects (Svelte, React, Python, Rust, …) without
> abusing `storage-fs` for it.
>
> **Driver.** [`docs/tasks/capabilities-task.md`](../tasks/capabilities-task.md)
> — too many near-duplicate storage embeddings dilute ANN signal and confuse
> the LLM. Consolidating yields higher accuracy, lower token cost, and the
> same hot-reload + per-tool testability. The `code-project` addition
> applies the same "one domain = one capability with many tools" rule to a
> new domain rather than scattering scaffold/edit/patch across `storage-fs`.
>
> **Architecture reference.** [`docs/capabilities/capabilities-arch.md`](capabilities-arch.md).
>
> **Revision 2026-05-21 (post-review).** Three canonical-drift fixes applied:
>
> 1. **No new abstraction.** The earlier `MultiOpProvider` wrapper +
>    `[[config.tools]]` manifest extension are dropped. Instead we
>    hand-write two purpose-built providers (`StorageWorkspaceProvider`,
>    `StorageFsProvider`) inside the existing
>    [`NativeStorageFactory`](../../apps/backend/crates/agent-core/src/capabilities/providers/native_storage.rs)
>    that each dispatch on `tool_name` in `invoke()`. The `ToolManifest`
>    schema is **unchanged** — only the standard `[[tools]]` blocks are used.
>    *(The earlier review draft referenced a `BuiltinFactory` in
>    `providers/builtin.rs`; that file does not exist in this repo — the real
>    home is `native_storage.rs`. Substance of the critique is adopted; the
>    location is corrected.)*
> 2. **`ArtifactBridge` owns materialisation.** Phase 8.A no longer adds a
>    `materialise_workspace_tree` tool. Instead, `scaffold_project` / `edit_file`
>    / `apply_patch` return a canonical `ToolOutput { content, artifacts: […] }`
>    where each generated file is one `Artifact { name, mime_type, data }`.
>    The existing [`ArtifactBridge`](../../apps/backend/crates/agent-core/src/bridge/artifact_bridge.rs)
>    already uploads to RustFS and writes to `WorkspaceContentStore` post-invoke —
>    no capability code touches workspace materialisation.
> 3. **Tightened effort.** Storage consolidation now 3 AI-hours (was 4–5);
>    `code-project` now 2.5 AI-hours (was 2).

---

## 1. Current state (audit)

### 1.1 Inventory — the 15 capabilities to consolidate

All are `kind = "native"`; each TOML has a single `[config] op = "<dispatch_key>"`
that the [`NativeStorageFactory`](../../apps/backend/crates/agent-core/src/capabilities/providers/native_storage.rs#L1247)
matches against. The factory already contains every provider implementation —
no Rust logic needs to be rewritten.

| Current cap dir                                                                                                    | Namespace                       | `[config] op`        | Tools (single each, except `storage-workspace`)        |
| ------------------------------------------------------------------------------------------------------------------ | ------------------------------- | -------------------- | ------------------------------------------------------ |
| [storage-workspace](../../apps/backend/capabilities/storage-workspace/capability.toml)                             | `storage.workspace`             | `workspace`          | `save_document`, `list_folders`                         |
| [storage-workspace-move](../../apps/backend/capabilities/storage-workspace-move/capability.toml)                   | `storage.node.move`             | `move_node`          | `move_node`                                            |
| [storage-create-folder](../../apps/backend/capabilities/storage-create-folder/capability.toml)                     | `storage.folder.create`         | `create_folder`      | `create_folder`                                        |
| [storage-ensure-folder](../../apps/backend/capabilities/storage-ensure-folder/capability.toml)                     | `storage.object.ensure_folder`  | `ensure_folder`      | `ensure_folder`                                        |
| [storage-ensure-date-folder](../../apps/backend/capabilities/storage-ensure-date-folder/capability.toml)           | `storage.ensure_date_folder`    | `ensure_date_folder` | `ensure_date_folder`                                   |
| [storage-find-by-name](../../apps/backend/capabilities/storage-find-by-name/capability.toml)                       | `storage.node.find_by_name`     | `find_by_name`       | `find_by_name`                                         |
| [storage-show-tree](../../apps/backend/capabilities/storage-show-tree/capability.toml)                             | `storage.tree.show`             | `show_tree`          | `show_tree`                                            |
| [storage-list-folders](../../apps/backend/capabilities/storage-list-folders/capability.toml)                       | `storage.object.list`           | `list_folders`       | `list_folders` *(filesystem-prefix listing)*           |
| [storage-delete](../../apps/backend/capabilities/storage-delete/capability.toml)                                   | `storage.node.delete`           | `delete_node`        | `delete_node`                                          |
| [storage-bulk-delete](../../apps/backend/capabilities/storage-bulk-delete/capability.toml)                         | `storage.node.bulk_delete`      | `bulk_delete`        | `bulk_delete`                                          |
| [storage-tag](../../apps/backend/capabilities/storage-tag/capability.toml)                                         | `storage.object.tag`            | `tag_object`         | `tag_object`                                           |
| [storage-put](../../apps/backend/capabilities/storage-put/capability.toml)                                         | `storage.put`                   | `put_object`         | `put_object`                                           |
| [storage-move](../../apps/backend/capabilities/storage-move/capability.toml)                                       | `storage.object.move`           | `move_object`        | `move_object`                                          |
| [storage-read-text](../../apps/backend/capabilities/storage-read-text/capability.toml)                             | `storage.fs.read`               | `read_text`          | `read_file`                                            |
| [storage-write-text](../../apps/backend/capabilities/storage-write-text/capability.toml)                           | `storage.fs.write`              | `write_text`         | `write_file`                                           |
| [file-storage](../../apps/backend/capabilities/file-storage/capability.toml) *(MCP, **keep**)*                     | `storage.object`                | n/a (mcp endpoint)   | `upload_file`, `download_file`, `presigned_url`        |

### 1.2 Code-level facts confirmed by audit

- `NativeStorageFactory::create()` dispatches **per `[config] op`** — there is
  no constraint that one capability == one op. We can make the factory honour
  a `[[config.tools]]` table that maps `tool_name → op`, or simpler: have each
  provider accept its tool name in `invoke()` (most already do — see
  `ReadTextProvider` accepting both `"read_file"` and `"read"`).
- `CapabilityRegistry` is keyed by `manifest.name` — collapsing capabilities
  is a pure manifest-side refactor.
- `SemanticCapabilityRouter` re-embeds on `register/replace`; deleting old
  dirs + adding new ones just produces fewer, richer embeddings.
- `ToolExecutor::tool_definitions_from_manifest` already emits
  `cap__tool` names from `[[tools]]` blocks — multi-tool manifests are first-class.
- Three callers register `NativeStorageFactory` (
  [`state.rs:200`](../../apps/backend/crates/agent-gateway/src/state.rs#L200),
  [`state.rs:383`](../../apps/backend/crates/agent-gateway/src/state.rs#L383),
  [`capability_routing.rs`](../../apps/backend/crates/agent-gateway/tests/capability_routing.rs))
  — none reference individual capability names; they rely on `kind = "native"`.
- `convert.audio_to_text` (two manifests sharing namespace) is **out of scope**
  here; that duplicate is handled separately by `transcribe-video` being
  `enabled = false` already.

### 1.3 What must **not** change

- Every existing tool name (`save_document`, `list_folders`, `show_tree`,
  `move_node`, `delete_node`, `bulk_delete`, `find_by_name`, `create_folder`,
  `ensure_folder`, `ensure_date_folder`, `put_object`, `move_object`,
  `tag_object`, `read_file`, `write_file`) — these may appear in user prompts,
  realtime UIs, audit logs, and existing PlanStep blobs in the DB.
- The Anthropic-safe joined name `{cap}__{tool}` will change (e.g.
  `storage_show_tree__show_tree` → `storage_workspace__show_tree`). Since these
  are derived per-turn and never persisted, this is safe.
- The `file-storage` MCP capability stays — different concern (RustFS/S3 object
  storage) and a different `kind`.

---

## 2. Target design

### 2.1 Two domain capabilities

1. **`storage-workspace`** (`kind = "native"`, namespace `storage.workspace`)
   The user-facing workspace toolkit. **11 tools** that all operate on
   `WorkspaceStore` nodes (ULIDs, folders, named documents):
   `save_document`, `list_folders` *(workspace top-level)*, `show_tree`,
   `find_by_name`, `create_folder`, `ensure_folder`, `ensure_date_folder`,
   `move_node`, `delete_node`, `bulk_delete`, `tag_object`.

2. **`storage-fs`** (`kind = "native"`, namespace `storage.fs`)
   The low-level filesystem toolkit operating on **paths** under the tenant
   workspace root. **5 tools**:
   `read_file`, `write_file`, `put_object`, `move_object`,
   `list_folders` *(filesystem-prefix listing; renamed → `list_paths` to avoid
   colliding with the workspace tool of the same name — see §4.2)*.

3. **`file-storage`** (`kind = "mcp"`) — **unchanged**. Stays as the
   RustFS/S3 object-storage capability.

> **Why two not one.** The router must surface `storage-workspace` for
> "save my notes" and `storage-fs` for "write to uploads/2026/05/x.bin".
> Mixing path-based and node-based tools into one card creates a different
> kind of confusion — the LLM frequently picks the wrong primitive. Two
> distinct mental models = two embeddings.

### 2.2 Multi-tool dispatch — two purpose-built providers, no new abstraction

A capability with multiple tools is just one `CapabilityProvider` whose
`invoke(tool_name, …)` switches on `tool_name`. We add **two purpose-built
providers** directly inside
[`native_storage.rs`](../../apps/backend/crates/agent-core/src/capabilities/providers/native_storage.rs)
and have `NativeStorageFactory::create()` instantiate them by matching
`manifest.name`. No wrapper, no `HashMap`, no manifest schema extension.

```rust
pub struct StorageWorkspaceProvider {
    manifest: ToolManifest,
    // shared deps: WorkspaceStore, WorkspaceContentStore, tenant root
}

#[async_trait]
impl CapabilityProvider for StorageWorkspaceProvider {
    fn manifest(&self) -> &ToolManifest { &self.manifest }
    async fn invoke(&self, tool_name: &str, input: &Value, tenant: Option<&TenantContext>)
        -> anyhow::Result<Value>
    {
        match tool_name {
            "save_document"       => self.save_document(input, tenant).await,
            "list_folders"        => self.list_folders(input, tenant).await,
            "show_tree"           => self.show_tree(input, tenant).await,
            "find_by_name"        => self.find_by_name(input, tenant).await,
            "create_folder"       => self.create_folder(input, tenant).await,
            "ensure_folder"       => self.ensure_folder(input, tenant).await,
            "ensure_date_folder"  => self.ensure_date_folder(input, tenant).await,
            "move_node"           => self.move_node(input, tenant).await,
            "delete_node"         => self.delete_node(input, tenant).await,
            "bulk_delete"         => self.bulk_delete(input, tenant).await,
            "tag_object"          => self.tag_object(input, tenant).await,
            other => anyhow::bail!("unknown tool '{other}' for storage-workspace"),
        }
    }
}

pub struct StorageFsProvider { /* mirror, 5 arms */ }
```

Each arm calls a free function that wraps the exact existing logic from the
legacy per-op providers (e.g. `save_document` arm calls the body currently
living in `WorkspaceNativeProvider`). Behaviour is byte-identical.

Factory dispatch becomes a two-line addition:

```rust
match manifest.name.as_str() {
    "storage-workspace" => Ok(Arc::new(StorageWorkspaceProvider::new(card, deps))),
    "storage-fs"        => Ok(Arc::new(StorageFsProvider::new(card, deps))),
    _ => /* existing single-op match on config.op for any remaining legacy caps */,
}
```

The legacy `[config] op = …` branch stays in place purely as a rollback safety
net during the migration window and is deleted at the end of Phase 7.

---

## 3. Step-by-step migration

### Phase 0 — Pre-flight (≈ 15 min)

- [ ] **0.1** Confirm no DB row in `capability_specs` references any of the
      15 capability names (these are all file-based natives; should be zero
      rows). Query: `SELECT name FROM capability_specs WHERE name LIKE 'storage-%';`.
- [ ] **0.2** Snapshot current behaviour: run
      `cargo test -p agent-gateway --test capability_routing -- --nocapture`
      and store the output to baseline assertions before/after.
- [ ] **0.3** `git checkout -b refactor/storage-capabilities-consolidation`.

### Phase 1 — Two purpose-built providers (≈ 1 h, code-only, no behaviour change)

- [ ] **1.1** In
      [`native_storage.rs`](../../apps/backend/crates/agent-core/src/capabilities/providers/native_storage.rs):
      extract every legacy per-op `invoke()` body into free `async fn`s
      (`save_document(…)`, `show_tree(…)`, `read_text(…)`, …) that take the
      shared deps as parameters. Pure mechanical extraction.
- [ ] **1.2** Add `StorageWorkspaceProvider` and `StorageFsProvider` structs
      with `CapabilityProvider` impls whose `invoke()` matches on `tool_name`
      and delegates to the extracted free functions (see §2.2).
- [ ] **1.3** Extend `NativeStorageFactory::create()` to match on
      `manifest.name` first; fall through to the existing single-op `match op`
      branch for any legacy capability still on disk during the migration.
- [ ] **1.4** Unit test `storage_workspace_dispatches_all_eleven_tools` and
      `storage_fs_dispatches_all_five_tools` that build the manifest, register
      it in an isolated `CapabilityRegistry`, and assert each tool's `invoke()`
      path returns the expected shape.
- [ ] **1.5** `cargo build -p agent-core` — green.

### Phase 2 — Author consolidated manifests (≈ 1 h, additive only)

> Manifests are added in *new* directories so the old ones still load. The
> registry will reject duplicate `name`s — we choose names that do not collide:
> the new files use the same canonical names (`storage-workspace`, `storage-fs`),
> which means the **legacy `storage-workspace` directory must be moved aside
> simultaneously**. We handle this in Phase 3.

- [ ] **2.1** Create
      `apps/backend/capabilities/storage-workspace/capability.toml.new` with
      `[[tools]]` × 11 (see appendix A.1). **No `[[config.tools]]` table** —
      the manifest stays vanilla; provider dispatch is by tool name in Rust.
      Tools: `save_document`, `list_folders`, `show_tree`, `find_by_name`,
      `create_folder`, `ensure_folder`, `ensure_date_folder`, `move_node`,
      `delete_node`, `bulk_delete`, `tag_object`.
- [ ] **2.2** Create
      `apps/backend/capabilities/storage-fs/capability.toml` with `[[tools]]`
      × 5 (see appendix A.2). Tools: `read_file`, `write_file`, `put_object`,
      `move_object`, `list_paths` *(renamed from `list_folders` to disambiguate
      from the workspace tool; see §4.2)*.
- [ ] **2.3** Write rich `description`, `tags`, and `search_keywords` blocks
      that union the keywords from every legacy capability — the embedding
      must dominate ANN recall for every previous query (see appendix A.3 for
      keyword sets).
- [ ] **2.4** Leave `accepts = ["application/json"]` and
      `emits = ["application/json"]` consistent across both. Set
      `cost_hint = "low"`, `idempotent = true` on `storage-fs` and `false` on
      `storage-workspace` (matches old per-cap values).

### Phase 3 — Cutover (≈ 30 min, single commit)

- [ ] **3.1** Delete the 15 legacy directories in one commit:
      ```
      git rm -r apps/backend/capabilities/{storage-workspace-move,storage-put,
        storage-read-text,storage-write-text,storage-move,storage-delete,
        storage-bulk-delete,storage-list-folders,storage-create-folder,
        storage-ensure-folder,storage-ensure-date-folder,storage-find-by-name,
        storage-show-tree,storage-tag}
      ```
- [ ] **3.2** Replace
      `apps/backend/capabilities/storage-workspace/capability.toml` with the
      `.new` file from step 2.1.
- [ ] **3.3** Commit the new `storage-fs` directory from step 2.2.
- [ ] **3.4** `cargo build` — verify no compile errors (none expected, all
      code paths still live in `native_storage.rs`).

### Phase 4 — Update tests (≈ 45 min)

- [ ] **4.1**
      [`capability_routing.rs`](../../apps/backend/crates/agent-gateway/tests/capability_routing.rs):
      replace any references to legacy capability names with the new ones.
      Tool names stay the same.
- [ ] **4.2** Run the e2e capability tour
      ([`docs/verify/verify.md`](../verify/verify.md)) and update any
      asserted capability lists.
- [ ] **4.3** Add a regression test
      `storage_workspace_exposes_all_legacy_tools` that registers the new
      manifest in an isolated `CapabilityRegistry`, lists tools, and asserts
      the 11-tool surface.
- [ ] **4.4** Add `storage_fs_renames_list_folders_to_list_paths` to ensure
      the rename is intentional and discoverable.

### Phase 5 — Router & embeddings (≈ 15 min, runtime check)

- [ ] **5.1** Start the stack (`./start.sh`); confirm gateway logs:
      - exactly two new cards loaded (`storage-workspace`, `storage-fs`),
      - **zero `WARN factory create failed`** for the new manifests,
      - the `capability.reloaded` event fires for both.
- [ ] **5.2** `curl /v1/capabilities` — verify only 23 cards remain
      (37 - 15 + 2 = 24; minus already-disabled `transcribe-video` and
      `google-workspace` = 22 enabled).
- [ ] **5.3** Verify Qdrant point count: `curl …/collections/conusai-capabilities`
      reports the lower vector count.

### Phase 6 — Behavioural verification (≈ 1 h)

- [ ] **6.1** Run the four canonical upload-pipeline e2e flows:
      upload-PDF → plan → OCR → classify → **save_document** → list_folders.
- [ ] **6.2** Run a workspace-management chat:
      "show me everything in my workspace" → expect `show_tree`;
      "delete the receipts folder" → expect `delete_node`;
      "find my CV" → expect `find_by_name`.
- [ ] **6.3** Run a low-level path flow:
      "write 'hello' to notes/scratch.txt" → expect `storage-fs.write_file`.
- [ ] **6.4** Compare router top-K for the prompt "save these notes" before
      vs after — only **`storage-workspace`** should appear (previously 3–4
      storage cards competed).

### Phase 7 — Docs & cleanup (≈ 30 min)

- [ ] **7.1** Update [`capabilities-arch.md`](capabilities-arch.md) §5.6 —
      replace the 15-row table with the 3-row table (workspace, fs, file-storage).
- [ ] **7.2** Update [`how-to-add-a-domain.md`](how-to-add-a-domain.md) with
      the new "multi-tool native via `[[config.tools]]`" pattern.
- [ ] **7.3** Add a one-line ADR addendum to
      [`adr/0007-everything-is-a-capability.md`](../adr/0007-everything-is-a-capability.md):
      *"Capabilities are *domain-level*. One capability ≡ one coherent
      toolkit; granularity lives in `[[tools]]`, not in directories."*
- [ ] **7.4** Open PR; reference this plan and the task doc.

### Phase 8 — New `code-project` capability (≈ 2 h, additive)

> Lands **after** the storage refactor is merged so the two changes don't
> intermix. Uses the exact same patterns (`MultiOpProvider` if native, rich
> `search_keywords`, single domain card).

**Why a new capability and not just `storage-fs`.** "Scaffold a Svelte app"
and "save my notes" are categorically different intents. A dedicated
embedding makes the router pick the right card on the first turn, and the
capability can offer **composite, multi-file tools** that a single
`write_file` call cannot — atomic project creation, file patches,
dependency edits.

**Design (kind = `chain`, no new Rust required).** The capability is pure
TOML. Each tool's chain emits a structured file tree; the agent loop then
calls `storage-fs.write_file` per entry (or emits a `PlanStep[]` consumed by
`run_plan`). An optional small native helper (`code-fs.write_tree`) can
collapse N writes into one audited call later.

- [ ] **8.1** Create `apps/backend/capabilities/code-project/capability.toml`:
      - `name = "code-project"`, `namespace = "code.project"`, `category = "compose"`, `kind = "chain"`.
      - Description anchor: *"Author and edit code projects: scaffold a new
        application in any supported framework, edit existing source files,
        apply patches, manage package dependencies. Operates on directories
        of source files under a tenant workspace path."*
      - Tools:
        - `scaffold_project(framework, name, target_path, description)`
          — `framework` enum: `sveltekit | vite-svelte | vite-react | nextjs |
          nuxt | node-cli | python-uv | rust-bin | go-cli`. Returns
          `{ files: [{path, content}], post_install: ["pnpm install", "pnpm dev"] }`.
        - `edit_file(path, instruction)` — read existing file, ask LLM for
          new contents, return updated content (agent loop writes it back).
        - `apply_patch(path, unified_diff)` — apply a unified diff to a file.
        - `add_dependency(package_json_path, name, version?, dev?)` — JSON-patch helper.
        - `read_project(target_path, max_files?)` — emit a structured
          summary of the project (tree + key file contents) for follow-up edits.
      - `output_schema` per tool enforces shape so malformed responses fail typed.
      - `cost_hint = "medium"`, `idempotent = false`.
      - `search_keywords`: `build app, create app, scaffold, new project,
        svelte, sveltekit, react, vite, next.js, nuxt, python project, rust
        project, edit code, add component, add route, add page, install
        package, add dependency, refactor, fix bug, update file, patch file,
        package.json, tsconfig, vite.config`.
- [ ] **8.2** Pick the model: `model = "smart"` for `scaffold_project` and
      `edit_file` (Opus); `model = "fast"` for `add_dependency` (Haiku — pure JSON edit).
- [ ] **8.3** Wire the framework templates *into the system prompt itself*
      (no on-disk templates) — the chain LLM generates the file tree from
      its training data. Pin model versions to keep scaffolds reproducible.
- [ ] **8.4** Add an integration test: chat prompt "create a minimal
      SvelteKit app under projects/demo-app" must produce ≥ 6 files under
      `projects/demo-app/` and the router must pick `code-project` (not
      `storage-fs`).
- [ ] **8.5** Update [`capabilities-arch.md`](capabilities-arch.md) §5.4 (compose)
      or §5.9 (a new `code` row depending on where it fits the taxonomy).
      Likely promote `code` to a first-class taxonomy root — propose in
      [`taxonomy.md`](taxonomy.md).
- [ ] **8.6** *(Optional follow-up, not blocking)* Add a tiny native
      `code-fs.write_tree(files: [{path, content_or_base64}])` provider in
      `native_storage.rs` that atomically writes a file tree in one
      auditable call. Drops scaffold tool-call count from ~10 to 1.

#### 8.A Filesystem vs. WorkspaceStore — `ArtifactBridge` owns materialisation

The platform has **two layers**:

- **Filesystem / object store** — where bytes live.
- **WorkspaceStore + WorkspaceContentStore** — DB-tracked nodes that the UI
  workspace tree displays.

The canonical bridge between them is
[`ArtifactBridge`](../../apps/backend/crates/agent-core/src/bridge/artifact_bridge.rs)
(see also [`common::artifact::ToolOutput`](../../apps/backend/crates/common/src/artifact.rs)).
After every `CapabilityProvider::invoke()` the gateway calls
`ArtifactBridge::process_if_artifacts()`, which:

1. Uploads each `Artifact { name, mime_type, data }` to the object store.
2. Writes indexable text (`text/*`, `application/pdf`, `application/json`) to
   `WorkspaceContentStore` — which is what the UI tree reads.

**Capabilities do not materialise. They return artifacts. `ArtifactBridge`
materialises.** This is a strict ownership rule in `arch.md`.

**Decision: `scaffold_project` / `edit_file` / `apply_patch` return
`ToolOutput.artifacts` and let `ArtifactBridge` do the rest.**

- [ ] **8.A.1** `scaffold_project`'s chain output schema returns:
      ```json
      {
        "content": "Scaffolded SvelteKit app under projects/demo-app (12 files).",
        "artifacts": [
          { "name": "projects/demo-app/package.json",
            "mime_type": "application/json",
            "data": "<base64>" },
          { "name": "projects/demo-app/src/routes/+page.svelte",
            "mime_type": "text/plain",
            "data": "<base64>" }
          // … one Artifact per generated file
        ]
      }
      ```
      The chain emits `content` as base64 (already the `Artifact` convention).
- [ ] **8.A.2** `edit_file` and `apply_patch` return a single-element
      `artifacts` array with the updated file. `ArtifactBridge` upserts via
      `WorkspaceContentStore.write(tenant, virtual_path, text)`, which is
      idempotent on `(tenant, path)`.
- [ ] **8.A.3** Tiny extension to `ArtifactBridge` (separate small PR): allow
      the caller to override the virtual path prefix so artifacts produced by
      `code-project` land at `/<artifact.name>` (the raw project path) rather
      than the default `/outputs/<tool_name>/<artifact.name>`. One-line
      addition to `materialise()`; preserves existing callers' behaviour.
- [ ] **8.A.4** Document the rule in [`capabilities-arch.md`](capabilities-arch.md):
      *“Capabilities never write workspace nodes directly. They return
      `ToolOutput.artifacts`; `ArtifactBridge` is the sole writer.”*
- [ ] **8.A.5** Drop the previously proposed `materialise_workspace_tree`
      tool from `code-project` — it is redundant with `ArtifactBridge`.

**Out of scope for Phase 8** (deliberately): running `pnpm install` /
`pnpm dev` or any subprocess. The platform remains a file-and-LLM
substrate; the user runs the dev server locally. A future `code-shell`
capability could fill this gap, but it needs sandboxing and resource
caps we don't yet have.

---

## 4. Risks, edge cases & rollback

### 4.1 Risks

| Risk                                                                                | Mitigation                                                                                                    |
| ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Hard-coded references to old `name` in tests, fixtures, or e2e scripts              | Phase 0.1 + 4.1 scan; `git grep -E 'storage-(put|read-text|write-text|move|delete|…)'` must return only docs/plan refs. |
| Persisted PlanStep blobs in `capability_specs` referencing old names                | Phase 0.1 query; if non-zero, write a one-off SQL migration mapping old → new.                                |
| `list_folders` ambiguity (workspace vs filesystem) confuses the LLM                  | Rename the fs variant to `list_paths` (Phase 2.2); keep workspace `list_folders` as the canonical phrase.     |
| MultiOpProvider double-builds inner providers each call                              | Build once in factory `create()`, store in `HashMap` for the cap's lifetime — same lifecycle as today.        |
| Router cache returns stale top-K for cached prompts                                  | Moka cache key includes capability set hash; `replace()` invalidates implicitly. Force-clear on boot.         |

### 4.2 Naming conflict resolution

Two tools both want `list_folders`. Decision:

- **Keep `list_folders` on `storage-workspace`** — most natural for "what
  folders do I have?" queries (workspace nodes).
- **Rename fs variant `list_folders` → `list_paths`** — clearer for "list
  files under uploads/2026/" path queries. Document in capability description.

### 4.3 Rollback

The change is two files (new manifests) plus 15 deletions and one factory
patch. Rollback = `git revert <commit>`; the legacy directories come back and
the factory still understands the old single-op manifests (Phase 1 only adds
a new code path, doesn't remove the old one).

---

## 5. Effort & success criteria

**Effort** *(revised 2026-05-21 after dropping `MultiOpProvider` + materialise tool)*:
- Phases 0–7 (storage consolidation): **3 AI-hours**, ≈ 65 k tokens.
  (1 h provider extraction + 1 h manifests + 0.5 h tests + 0.5 h verification/docs.)
- Phase 8 (`code-project` + `ArtifactBridge` prefix override): **2.5 AI-hours**, ≈ 45 k tokens. Separate PR.

**Success criteria — storage consolidation (Phases 0–7)**:

1. `cargo test --workspace` green.
2. Gateway boot logs show exactly **3 storage cards** (workspace, fs, file-storage).
3. Manual chat: "save these notes to Research" picks `storage-workspace.save_document` on first try, 10/10 runs.
4. Router top-K for any storage prompt returns ≤ 2 storage cards (currently 4–6).
5. Total `/v1/capabilities` count drops by 13 (15 removed → 2 added).
6. Tool-name set exposed to the LLM is unchanged except for the documented
   `list_folders → list_paths` rename on the fs side.

**Success criteria — `code-project` (Phase 8)**:

7. Chat: "create a minimal SvelteKit app under projects/demo-app" produces
   a working source tree (`package.json`, `vite.config.ts`, `src/routes/+page.svelte`, …).
8. Router picks `code-project.scaffold_project` (not `storage-fs.write_file`) for that prompt, 10/10 runs.
9. Chat: "add lodash to projects/demo-app/package.json" picks
   `code-project.add_dependency`, not a generic LLM monologue.

---

## Appendix A — Manifest skeletons

### A.1 `storage-workspace/capability.toml`

```toml
schema_version = "2.0"
name        = "storage-workspace"
version     = "2.0.0"
namespace   = "storage.workspace"
category    = "storage"
kind        = "native"
description = """
The user's workspace toolkit. Save, organise, find, move, tag and delete
documents and folders in the tenant's hierarchical workspace.
Use these tools whenever the user talks about their files, folders, notes,
documents, or workspace in natural language.
"""
tags     = ["workspace", "storage", "files", "folders", "documents"]
accepts  = ["application/json"]
emits    = ["application/json"]
idempotent = false
cost_hint = "low"
requires  = []
search_keywords = [
  "save", "store", "save as", "save document", "save note", "save file",
  "folder", "new folder", "create folder", "make folder",
  "list folders", "what folders", "show workspace", "show tree", "outline",
  "find", "find file", "lookup", "search workspace",
  "move", "rename", "relocate", "move file",
  "delete", "remove", "trash", "empty folder", "bulk delete",
  "tag", "label", "annotate",
  "ensure folder", "today's folder", "date folder",
]

[[tools]]
name = "save_document"
description = "Save text content as a document in a workspace folder…"
# … input_schema unchanged from legacy storage-workspace …

[[tools]]
name = "list_folders"
description = "List top-level workspace folders available to the user."
# …

[[tools]]
name = "show_tree"
description = "Render a Markdown tree of folders and files under parent_id (or root)."
# …

# … 8 more [[tools]] blocks, one per legacy capability …

# NOTE: No [[config.tools]] table. Provider dispatch lives in
# `StorageWorkspaceProvider::invoke()` (see §2.2). The manifest stays vanilla
# — `ToolManifest` schema is unchanged.
```

### A.2 `storage-fs/capability.toml`

```toml
schema_version = "2.0"
name        = "storage-fs"
version     = "1.0.0"
namespace   = "storage.fs"
category    = "storage"
kind        = "native"
description = """
Low-level filesystem operations on paths under the tenant workspace root.
Use when the user gives an explicit path (e.g. "uploads/2026/05/file.pdf"),
not when they talk about workspace nodes by name.
"""
tags     = ["storage", "filesystem", "path", "read", "write"]
accepts  = ["application/json"]
emits    = ["application/json"]
idempotent = true
cost_hint = "low"
search_keywords = [
  "read file by path", "write file by path", "put file", "upload to path",
  "list paths", "list under prefix", "move file path",
]

[[tools]]
name = "read_file"
# …

[[tools]]
name = "write_file"
# …

[[tools]]
name = "put_object"
# …

[[tools]]
name = "move_object"
# …

[[tools]]
name = "list_paths"
description = "List files and directories under a path prefix in the workspace root."
# (Renamed from legacy `list_folders` to disambiguate from
# `storage-workspace.list_folders`.)

# NOTE: No [[config.tools]] table. Provider dispatch lives in
# `StorageFsProvider::invoke()` (see §2.2).
```

### A.3 Search-keyword unions

When merging, **union** every legacy capability's `tags` and `search_keywords`
into the new manifest. This guarantees the consolidated card's embedding
strictly dominates any legacy card's embedding for any historical query.

---

## Appendix B — Open questions (not blocking)

1. **Should `file-storage` (MCP) also expose presigned URLs from
   `storage-workspace`?** Probably no — different cost tier, different
   credential surface. Keep separate.
2. **Should we also consolidate `extract-ocr-vision` + `ocr-service`?** Yes,
   in a follow-up. Same pattern (two near-duplicate chains) — out of scope
   for this PR.
3. **`plan-orchestrate` model selection** — unchanged.
4. **Future**: introduce a `kind = "toolkit"` synonym for `native` when the
   capability has > 5 tools, purely for documentation clarity.
5. **`ArtifactBridge` `parent_node_id` plumbing.** `process_if_artifacts`
   accepts `parent_node_id` but the current `materialise()` does not use it
   when computing `virtual_path`. Phase 8.A.3 plumbs that through so
   scaffold trees can attach under a chosen parent folder node.
