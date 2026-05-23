# Capability Taxonomy

Every capability in ConusAI must declare a `namespace` (dot-separated slug) and a
`category` from the table below. Both appear in the TOML manifest, both are used
by the `SemanticCapabilityRouter` to route intents, and both are enforced by
`cargo xtask capabilities lint`.

---

## Namespace Table

| Category  | Namespace prefix | Examples                                                                   | Typical kind              |
|-----------|------------------|----------------------------------------------------------------------------|---------------------------|
| Storage   | `storage.*`      | `storage.workspace`, `storage.fs`, `storage.object`                       | `native` / `mcp`          |
| Compute   | `compute.*`      | `compute.run_cargo`, `compute.shell` (gated)                               | `native` (opt-in)         |
| Sense     | `sense.*`        | `sense.mime`, `sense.detect_language`, `sense.classify_document`           | `chain` or `wasm`         |
| Extract   | `extract.*`      | `extract.ocr.vision`, `extract.ocr.tesseract`, `extract.fields.invoice`, `extract.fields.contract` | `chain`  |
| Convert   | `convert.*`      | `convert.pdf_to_md`, `convert.audio_to_text`, `convert.image_to_thumb`    | `chain` / `wasm` / job    |
| Compose   | `compose.*`      | `compose.email`, `compose.invoice_pdf`, `compose.report_md`               | `chain`                   |
| Deliver   | `deliver.*`      | `deliver.email_smtp`, `deliver.webhook`, `deliver.s3_export`              | `remote_mcp` / `native`   |
| Plan      | `plan.*`         | `plan.orchestrate`, `plan.route_by_mime`, `plan.on_upload`                | `chain` (meta)            |
| Code *(proposed)* | `code.*` | `code.project` — scaffold/edit/patch code projects                  | `chain`                   |

> **`code` root status.** `code.project` is in use by
> [code-project](../../apps/backend/capabilities/code-project/capability.toml)
> with `category = "compose"` as a temporary assignment. When the `code` domain grows
> (e.g. `code-shell`, `code-test`, `code-review`), promote `code` to a first-class root
> and update this table, the lint allowlist in `xtask/src/main.rs`, and the existing
> `code-project` manifest.

**Rule:** every new domain element MUST pick a namespace from this taxonomy.
Adding a new category requires updating this table, `docs/capability-authoring-guide.md`,
and the lint allowlist in `xtask/src/main.rs`.

---

## Rules

1. **Namespace uniqueness.** No two capabilities may share the same `namespace` value.
   The namespace identifies the logical operation family, not the implementation.

2. **`category` must match namespace root.** A capability with `namespace = "extract.fields.invoice"`
   must declare `category = "extract"`.

3. **TOML > Rust for new domains.** If a capability can be expressed as a prompt chain,
   it MUST live as a `kind = "chain"` manifest. Native Rust providers are reserved for
   storage, compute, and system integrations.

4. **`accepts` and `emits` are required for extract/convert/sense.** These fields power
   the router's MIME post-filter (Phase 2.1) and the planner's chaining suggestions.

5. **`cost_hint` is strongly recommended.** The `plan.orchestrate` meta-capability uses it
   to rank strategies. Use `dollars` if you know the approximate per-call cost.

---

## Worked Examples

### `extract.fields.invoice`
```toml
name        = "extract-fields-invoice"
version     = "1.0.0"
namespace   = "extract.fields.invoice"
category    = "extract"
kind        = "chain"
accepts     = [{ mime = "application/pdf" }, { mime = "image/*" }]
emits       = ["application/json"]
idempotent  = true
cost_hint   = { dollars = 0.05, latency_ms = 4000 }

[chain]
model           = "smart"
vision          = true
max_tokens      = 2048
system_prompt   = "You are an invoice data extraction specialist..."
prompt_template = "Extract all invoice fields from the attached document..."
```

### `storage.put`
```toml
name        = "storage-put"
version     = "1.0.0"
namespace   = "storage.put"
category    = "storage"
kind        = "native"
accepts     = [{ mime = "*" }]
emits       = ["application/json"]
idempotent  = false   # writing has side-effects
cost_hint   = { latency_ms = 200 }

[native]
op = "put_object"
```

### `sense.classify_document`
```toml
name        = "sense-classify-document"
version     = "1.0.0"
namespace   = "sense.classify_document"
category    = "sense"
kind        = "chain"
accepts     = [{ mime = "text/plain" }, { mime = "application/json" }]
emits       = ["application/json"]
cost_hint   = { dollars = 0.001 }

[chain]
model           = "cheap"
vision          = false
max_tokens      = 256
output_schema   = { type = "object", required = ["type", "confidence"], ... }
system_prompt   = "Classify the document type..."
prompt_template = "Document text: {{input.text}}\nClassify as one of: invoice, contract, cv, medical_claim, incident_report, other."
```

---

## Enforcement

`cargo xtask capabilities lint` validates every TOML under `apps/backend/capabilities/`:
- `namespace` is present and matches the `category` root.
- `accepts` and `emits` are non-empty for `extract`, `convert`, and `sense` categories.
- `kind = "chain"` manifests have a `[chain]` block.
- `schema_version` is `"2.0"`.

Wire this into CI: `cargo xtask capabilities lint` must pass before merge.
