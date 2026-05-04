**Full Implementation Plan: Scheduled Jobs + Background Tasks (v0.3-compliant)**

**Goal**  
Add production-grade scheduled jobs (`ScheduledJob`) and long-running background jobs (`BackgroundJob`) to the ConusAI platform without breaking any existing v0.3 invariants.  
The solution reuses the new `crates/jobs/` crate we defined earlier, extends it with a single `BackgroundJob` trait and `JobExecutor`, and provides an immediate real-world capability (`TranscribeVideoCapability`) so users can call “transcribe this video” and get an instant `task_id` + polling/SSE.

**Why this plan is optimal (May 2026)**  
- Zero new top-level crates (we evolve `crates/jobs/` only).  
- 100 % Rig.rs / Axum / Tokio alignment.  
- In-memory executor for MVP → one-line swap to Apalis + Postgres later.  
- Full observability (tracing + Prometheus + OpenTelemetry).  
- Follows every canonical name from the v0.3 table.  
- Total estimated effort: **28–36 AI-hours** (~210k tokens).

### Phase 0: Workspace Preparation (2–3 AI-hours)

1. **Update root `Cargo.toml`**  
   Add to `[workspace.dependencies]`:
   ```toml
   tokio-cron-scheduler = "0.15"
   ```

2. **Add new workspace member**  
   Create `crates/jobs/Cargo.toml` with:
   ```toml
   [package]
   name = "jobs"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true

   [dependencies]
   common = { path = "../common" }
   agent-core = { path = "../agent-core" }
   tokio = { workspace = true }
   tokio-cron-scheduler = { workspace = true }
   async-trait = { workspace = true }
   anyhow = { workspace = true }
   tracing = { workspace = true }
   uuid = { workspace = true }
   serde = { workspace = true }
   serde_json = { workspace = true }
   tokio-util = { workspace = true }   # for later SSE
   ```

3. **Update `apps/backend/Cargo.toml`**  
   Add under `[dependencies]`:
   ```toml
   jobs = { path = "../../crates/jobs" }
   ```

**Commit message**: `chore: add crates/jobs workspace member (tokio-cron-scheduler 0.15)`

### Phase 1: Core `crates/jobs` Implementation (8–10 AI-hours)

Create the following files exactly as named:

**`crates/jobs/src/lib.rs`**
```rust
pub mod job;
pub mod context;
pub mod registry;
pub mod scheduler;
pub mod executor;
pub mod admin;

pub use job::{ScheduledJob, BackgroundJob};
pub use context::JobContext;
pub use registry::JobRegistry;
pub use scheduler::JobSchedulerService;
pub use executor::JobExecutor;
pub use admin::JobAdmin;
```

**`crates/jobs/src/job.rs`** – define both traits (canonical names unchanged).

**`crates/jobs/src/context.rs`** – extend `JobContext` with `Arc<JobExecutor>`.

**`crates/jobs/src/registry.rs`** – `JobRegistry` now holds:
- `Vec<Arc<dyn ScheduledJob>>`
- `Vec<Arc<dyn BackgroundJob>>` (type-erased via a helper wrapper)
- `Arc<JobContext>`

**`crates/jobs/src/scheduler.rs`** – `JobSchedulerService` (unchanged from previous proposal).

**`crates/jobs/src/executor.rs`** – new `JobExecutor` with in-memory `RwLock<HashMap<Uuid, TaskStatus>>` + `enqueue` + `get_status` + `subscribe_sse`.

**`crates/jobs/src/admin.rs`** – `JobAdmin` trait (mirrors `CapabilityAdmin` exactly).

### Phase 2: Integration into `agent-core` & `agent-gateway` (6–8 AI-hours)

1. **agent-core**  
   - `crates/agent-core/src/capabilities/mod.rs` – re-export `jobs::BackgroundJob`.  
   - Add `TranscribeVideoCapability` in a new file `crates/agent-core/src/capabilities/transcribe_video.rs`.

2. **agent-gateway**  
   - Extend `AppState` with:
     ```rust
     pub job_registry: Arc<JobRegistry>,
     pub job_scheduler: Arc<JobSchedulerService>,
     pub job_executor: Arc<JobExecutor>,
     ```
   - In `main.rs` (or `startup.rs`):
     - Create `JobContext` → `JobRegistry` → `JobSchedulerService` → `JobExecutor`.
     - Spawn scheduler.
     - Register built-in jobs (see Phase 3).

3. **Protected router updates** (`crates/agent-gateway/src/router/protected.rs`):
   | Method | Path                        | Handler                  |
   |--------|-----------------------------|--------------------------|
   | GET    | `/v1/tasks`                 | `list_tasks`             |
   | GET    | `/v1/tasks/{id}`            | `get_task`               |
   | GET    | `/v1/tasks/{id}/sse`        | `task_sse` (optional)    |

4. **Admin router updates** (`crates/agent-gateway/src/router/admin.rs`):
   | Method | Path                        | Handler                  |
   |--------|-----------------------------|--------------------------|
   | GET    | `/admin/jobs`               | `list_jobs`              |
   | GET    | `/admin/jobs/{name}`        | `get_job`                |
   | PATCH  | `/admin/jobs/{name}/enable` | `toggle_job`             |
   | POST   | `/admin/jobs/{name}/run`    | `run_now`                |

### Phase 3: Built-in Jobs & First Capability (6–7 AI-hours)

1. **Scheduled jobs** (in `crates/jobs/src/jobs/`):
   - `capability_health_check.rs`
   - `audit_log_cleanup.rs`
   - `rag_reindex.rs` (uses `rig-qdrant`)

2. **Background jobs**:
   - `video_transcription.rs` – downloads from object_store, runs `whisper-rs` (or external Whisper API) inside `spawn_blocking`, stores `.txt` + `.vtt` back to object_store.

3. **Capability**:
   - `TranscribeVideoCapability` implements `CapabilityProvider`.
   - Accepts `file_id` (from previous `/v1/files` upload).
   - Enqueues `VideoTranscriptionJob` → returns `{ "task_id": "...", "status": "queued" }` instantly.

Register both job types in `JobRegistry::new()`.

### Phase 4: Observability, Tests & Docs (4–5 AI-hours)

- Add tracing spans + Prometheus counters (`jobs_executed_total`, `jobs_failed_total`, `job_duration_seconds`).
- Unit tests in `crates/jobs/tests/` (use `tokio::test` + mock `JobContext`).
- Integration test for `/v1/tasks` flow.
- Update ADR in `docs/adr/004-jobs-and-background-tasks.md`.
- Update OpenAPI spec (utoipa) and Askama UI panel for “Tasks” and “Jobs”.

### Phase 5: Validation & Merge Criteria

- Clippy + `cargo nextest` passes on Rust 1.88.
- All new code uses only workspace dependencies.
- No blocking calls in HTTP handlers.
- Manual test: upload video → call capability → poll task → receive transcript.
- Documentation: 1-page “How to add a new scheduled/background job” in `docs/`.

**Total Effort Breakdown**  
- Phase 0: 2–3 h  
- Phase 1: 8–10 h  
- Phase 2: 6–8 h  
- Phase 3: 6–7 h  
- Phase 4: 4–5 h  
**Grand total: 28–36 AI-hours (~210k tokens)**

**Risks & Mitigations**  
- Whisper CPU usage → already mitigated by `spawn_blocking`.  
- Memory growth in executor → in-memory only for MVP; Apalis migration path documented.  
- No breaking changes to existing routes/capabilities.

This plan is complete, self-contained, and ready for an AI (or human) to execute line-by-line.  

**Next step**  
Reply with “START IMPLEMENTATION” and I will generate the **first batch** of files (Phase 0 + Phase 1 complete code) as PR-ready diffs.  
Or say which phase you want first.