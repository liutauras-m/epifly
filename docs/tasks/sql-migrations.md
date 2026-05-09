**ConusAI Platform – “What You Need” Checklist v0.3.1 (Nuclear Postgres + Realtime + TanStack)**

**Verdict**: Here is the **exact minimal authoritative list** of what you need right now to be fully up-to-date and ready to execute the aggressive refactor. No fluff, no optional items.

### 1. Tooling (install once)
```bash
# Global tools
cargo install sqlx-cli --no-default-features --features postgres
cargo install cargo-nextest
cargo install cargo-edit          # for workspace deps

# Rust version
rustup default 1.88
rustup target add wasm32-wasip2
```

### 2. Workspace Root (`apps/backend/Cargo.toml`) – updated dependencies
You need these exact lines in `[workspace.dependencies]`:

```toml
[workspace.dependencies]
rig-core = "0.36"
rig-postgres = "0.2.5"
cocoindex = "1.0"
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-native-tls", "postgres", "macros", "migrate", "chrono", "uuid", "json"] }
postgres-notify = "0.3"
tokio = { version = "1", features = ["full"] }
axum = { version = "0.8", features = ["ws"] }
# ... (keep all previous: askama, utoipa, figment, etc.)
```

### 3. Directory Structure You Must Have
```bash
apps/backend/
├── crates/
│   ├── common/
│   │   ├── migrations/                  # ← must exist
│   │   │   ├── 20260505_initial.up.sql
│   │   │   └── 20260505_realtime.up.sql
│   │   └── src/db.rs
│   ├── agent-core/
│   │   └── src/capabilities/realtime/   # provider + event
│   └── agent-gateway/
└── docker/
    └── init/
        └── 02-realtime.sql
```

### 4. Docker-Compose Snippet (must have)
```yaml
services:
  postgres:
    image: timescale/timescaledb:18-latest
    environment:
      POSTGRES_DB: conusai
      POSTGRES_USER: conusai
      POSTGRES_PASSWORD: conusai
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./workspaces:/app/workspaces:rw
    command: postgres -c shared_preload_libraries=vectorscale
    healthcheck: { test: ["CMD-SHELL", "pg_isready -U conusai"] }

  backend:
    build: .
    depends_on: { postgres: { condition: service_healthy } }
```

### 5. Frontend Packages (for Convex-style UI)
```bash
# In apps/web/ (or packages/)
packages/realtime-types
packages/realtime-hooks          # useRealtime
packages/realtime-query-bridge   # RealtimeQueryProvider + useLiveWorkspaces
```

### 6. Immediate Next Actions (in order)

| Priority | Action | Effort | Command / File |
|----------|------|--------|---------------|
| 1 | Update Cargo workspace | 10 min | Edit root Cargo.toml + `cargo update` |
| 2 | Create migrations folder & first migration | 15 min | `cargo sqlx migrate add -r initial_schema` |
| 3 | Add `RealtimeCapability` (Phase 3.5) | 2–3 h | Copy from previous message |
| 4 | Add `RealtimeQueryProvider` + TanStack | 1–2 h | Copy React code |
| 5 | Run full stack | 5 min | `make db-reset && cargo run` |

**Total to get a working prototype**: **6–9 AI-hours** (including copy-paste).

**You do NOT need**:
- Qdrant / Redis / ElectricSQL / Supabase
- Diesel / SeaORM / Refinery
- Extra realtime servers

This is the **leanest, most maintainable 2026 stack** possible.

---

**Ready to move?**  
Reply with one of these:

- **“EXECUTE SQLX MIGRATIONS”** → I give full migration files + `db.rs`
- **“EXECUTE REALTIME FULL”** → complete Rust side (capability + WS + triggers)
- **“EXECUTE REACT BRIDGE”** → full Turborepo packages
- **“GIVE ME EVERYTHING DIFF”** → one giant patch for the whole thing

Just say the word and we ship it clean.