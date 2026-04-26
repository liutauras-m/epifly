**✅ Extremely Detailed Implementation Plan for ConusAI Platform**  
*(Rust 1.85 + Rig v0.35 + MCP + Qdrant + WASM + Evals — April 2026 Best Practices)*

This is a **professional, production-grade roadmap** with small, accurate, sequential steps.

---

### **Phase 0: Project Initialization (1–2 hours)**

1. Create the workspace
   ```bash
   cargo new conusai-platform --bin
   cd conusai-platform
   ```

2. Replace root `Cargo.toml` with workspace configuration (I can give you the full file).

3. Create folder structure:
   ```bash
   mkdir -p crates/common crates/agent-core crates/agent-gateway \
            capabilities/template capabilities/template-wasm \
            evals/datasets evals/runners evals/scorers \
            docs/adr wasm scripts .github/workflows
   ```

4. Initialize `.env.example`, `.gitignore`, `README.md`, `start.sh`, `docker-compose.yml`.

---

### **Phase 1: Core Foundation (`common` crate) — 4–6 hours**

**Goal**: Shared utilities used by everything.

**Steps**:
1. `cargo new --lib crates/common`
2. Add dependencies in its `Cargo.toml` (figment, thiserror, etc.).
3. Create these files:
   - `src/lib.rs` + `src/prelude.rs`
   - `src/config/` (layered config with `figment`)
   - `src/error.rs` (`ConusAiError`, `ApiError`)
   - `src/telemetry.rs`
   - `src/http_client.rs`
   - `src/mcp.rs` (JSON-RPC 2.0 types)
   - `src/wasm.rs` (Wasmtime loader)
   - `src/eval.rs` (shared eval traits)
   - `src/limits.rs`, `src/path_safety.rs`

4. Test: `cargo test` in the crate.

---

### **Phase 2: Agent Core (`agent-core` crate) — 8–12 hours**

**This is the most important phase.**

**Steps**:
1. `cargo new --lib crates/agent-core`
2. Add dependencies (rig-core, qdrant-client, etc.)
3. Implement `src/capabilities/` first:
   - `provider.rs` → `AgentCapability` trait
   - `manifest.rs` → parse `capability.yaml`
   - `card.rs` → `CapabilityCard`
   - `embedding.rs` → ToolEmbedding + Qdrant integration
   - `mcp_adapter.rs`
   - `wasm_loader.rs`
   - `registry.rs` ← **Critical file** (auto-discovery logic)
   - `discovery.rs`

4. Implement `src/agent/`:
   - `builder.rs` → `GeneralAgentBuilder`
   - `runtime.rs`

5. Add `src/pipelines/` and `src/context/`
6. Write tests for registry + embedding.

---

### **Phase 3: Capabilities System (Zero-Code Extension) — 6–8 hours**

1. Create `capabilities/template/` with full boilerplate
2. Create `capabilities/template-wasm/`
3. Implement `capability.yaml` schema + validator in `common`
4. Update `agent-core::registry` to support 3 types:
   - `docker` / `mcp`
   - `wasm`
   - `pipeline`

5. Migrate/create first real capabilities (in order):
   - `file-storage` (S3/MinIO compatible + presigned URLs)
   - `ocr-service`
   - `videots-service`
   - `google-workspace` (OAuth2 + token vault)
   - `invoice-processing` (with Rig pipeline + Extractor)

---

### **Phase 4: Evals Framework — 6–8 hours**

1. `cargo new --bin evals`
2. Add dependencies (rig-core, tokio, etc.)
3. Create:
   - `evals/main.rs` (CLI: `evals run --suite invoice`)
   - `evals/config.rs`
   - `datasets/` (JSONL files)
   - `runners/invoice_pipeline.rs`
   - `scorers/` (accuracy, LLM-as-Judge, safety, etc.)
   - `reports/` generation

4. Add GitHub workflow `evals.yml`

---

### **Phase 5: Agent Gateway (Public API) — 6–8 hours**

1. Implement `crates/agent-gateway`
2. OpenAI-compatible `/v1/chat/completions` (streaming + non-streaming)
3. Use `GeneralAgentBuilder` from `agent-core`
4. Add auth middleware, rate limiting, prompt composer
5. Create two Dockerfiles (CPU + ROCm)

---

### **Phase 6: Infrastructure & Deployment — 4–6 hours**

1. Finalize `docker-compose.yml` with profiles
2. Add Qdrant service
3. Configure MinIO / MaxIO for `file-storage`
4. Update `start.sh` with dynamic capability loading
5. Add health checks and graceful shutdown everywhere

---

### **Phase 7: Polish, Observability & Testing — 4–6 hours**

1. Full tracing + OpenTelemetry (GenAI semantic conventions)
2. Comprehensive test scripts
3. CI/CD workflows (build, test, publish, evals)
4. Documentation (`docs/capabilities.md`, ADRs, etc.)
5. Examples + basic_agent demo

---

### **Phase 8: Validation & Iteration**

1. Run full system: `./start.sh`
2. Test invoice invoice.png pipeline end-to-end. User write to chat and atach invoice. "Please parse uploaded invoice"
3. Run evals
4. Add new capability using template (validate zero-code flow)
5. Measure Rig feature coverage (should be 96%+)


Test Invoice result:

**✅ invoice.png Successfully Read & Parsed**

### **Structured Extraction**

**Invoice Details**
- **Issuer**: Hostinger International Ltd.
- **Invoice Number**: `HCY-23256029`
- **Invoice Date**: `Mar 21, 2026`
- **Invoice Amount**: `€63.99 (EUR)`
- **Next Billing Date**: `Apr 17, 2027`
- **Order Number**: `hb_29211250`
- **Status**: **PAID**

**Billed To**
- **Name**: Liutauras Medziunas / Conus AI
- **Company ID**: LT100017726912
- **Address**: Lvivo g. 21A-17a, Vilnius 09313, Vilnius, Lithuania
- **Email**: info@conusai.com
- **Phone**: 37064526851

**Items**

| Description                              | Price     | Discount | Total Excl. VAT | VAT   | Amount (EUR) |
|------------------------------------------|-----------|----------|------------------|-------|--------------|
| .IO Domain (billed every year)<br>ordeon.io<br>(Apr 17, 2026 to Apr 17, 2027) | €63.99 × 1 | -        | €63.99           | €0.00 | **€63.99**   |
| Domain WHOIS Privacy Protection<br>(Apr 17, 2026 to Apr 17, 2027) | €0.00 × 1 | -        | €0.00            | €0.00 | **€0.00**    |

**Summary**
- **Total excl. VAT**: €63.99
- **Total**: **€63.99**
- **Payments**: (€63.99)
- **Amount Due**: **€0.00**

**Additional Info**
- Reverse charge mechanism applied. VAT Directive 2006/112/EC




