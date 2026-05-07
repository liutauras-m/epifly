**# ConusAI Platform – Zero-Core-Touch Mode + Self-Registering Example Capability**  
**Implementation Plan v0.3.3**  
**Status**: Ready for AI implementation (Rust + Python)  
**Goal**: Add any new capability **without touching a single line of Rust code in `conusai-platform`** after the one-time foundation is in place.  
**Example capability**: A simple, separately-hosted `current-time` tool (returns ISO timestamp + optional timezone). Hosted as a tiny Python Docker service using MCP. Registers automatically on deploy. Supports **global enablement** (all tenants) or **specific tenants** via manifest.  
**Alignment**: 100 % SRP, `CapabilityProvider` + `CapabilityCard` + `CapabilityRegistrar` (already designed in v0.3.2), Postgres registry, `ArtifactBridge`, Rig.rs.  

**Zero-Core-Touch Definition**  
After this plan ships, any new capability follows this pattern:  
1. Deploy a new Docker service (Python/Rust/WASI).  
2. On startup → call `POST /admin/capabilities/register` with manifest.  
3. Done. No Rust rebuild, no Cargo changes, no platform restart required.

---

### Phase 0 – Platform Readiness Check (0 AI-hours for you, 1 AI-hour for AI coder)

**Prerequisites** (must be true before starting):
- Platform is at **v0.3.2** (or higher) with:
  - `CapabilityRegistrar` + `/admin/capabilities/register` endpoint (protected by super-admin JWT).
  - `CapabilityVectorStore` (pgvectorscale) + embedding on registration.
  - `ArtifactBridge` hooked in `CapabilityRouter` (for future file outputs).
  - `CapabilityCard` already supports `namespace`, `tenant_scope` (array of tenant_ids or `"global"` string), `enabled`.
- `CONUSAI_PLATFORM_TOKEN` (super-admin JWT) is available as secret.
- `docker-compose.yml` has profile `full` and MinIO/Postgres running.

**Validation command** (run once):
```bash
curl -H "Authorization: Bearer $PLATFORM_ADMIN_TOKEN" \
  http://localhost:8080/admin/capabilities \
  | jq '.[0].name'   # should return existing capabilities
```

If any piece is missing, first merge the minimal v0.3.2 registrar/bridge from previous plans (2 AI-hours max).

---

### Phase 1 – Create External Capability Service (3–4 AI-hours)

**Location**: New folder `./services/current-time` (outside `apps/backend/` — zero core touch).

**Files to create**:

#### 1.1 `services/current-time/Dockerfile`
```dockerfile
FROM python:3.12-slim
WORKDIR /app
RUN pip install fastapi uvicorn mcp python-jose[cryptography] httpx
COPY . .
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8082"]
```

#### 1.2 `services/current-time/main.py` (MCP + auto-registration)
```python
import os
import httpx
import base64
from datetime import datetime
from zoneinfo import ZoneInfo
from fastapi import FastAPI
from mcp.server.fastmcp import FastMCP
from mcp.types import ToolResult

mcp = FastMCP("current-time")

@mcp.tool()
async def get_current_time(timezone: str = "UTC") -> ToolResult:
    """Returns current ISO timestamp. Optional timezone (e.g. 'Europe/Helsinki')."""
    tz = ZoneInfo(timezone) if timezone != "UTC" else ZoneInfo("UTC")
    now = datetime.now(tz)
    return {
        "content": f"Current time: {now.isoformat()}",
        "artifacts": [],  # can add file output later via ArtifactBridge
        "metadata": {"timezone": timezone, "timestamp": now.isoformat()}
    }

app = FastAPI()
mcp.mount(app)

# ── AUTO-REGISTRATION (zero-core-touch magic) ──
async def register_capability():
    manifest = {
        "capability_id": "media.time.current-time",
        "name": "current-time",
        "namespace": "media.time",
        "description": "Returns current server time with optional timezone support",
        "version": "1.0.0",
        "kind": "RemoteMcp",
        "endpoint": "http://current-time:8082/mcp",
        "tools": [{
            "name": "get_current_time",
            "description": "...",
            "input_schema": {"type": "object", "properties": {"timezone": {"type": "string"}}}
        }],
        "tenant_scope": os.getenv("CONUSAI_TENANT_SCOPE", "global"),  # "global" or comma-separated tenant_ids
        "enabled": True
    }

    token = os.getenv("CONUSAI_PLATFORM_TOKEN")
    async with httpx.AsyncClient() as client:
        resp = await client.post(
            f"{os.getenv('CONUSAI_PLATFORM_URL')}/admin/capabilities/register",
            json=manifest,
            headers={"Authorization": f"Bearer {token}"}
        )
        resp.raise_for_status()
        print("✅ current-time capability registered")

if __name__ == "__main__":
    import asyncio
    asyncio.run(register_capability())
    # then start FastAPI
```

#### 1.3 `services/current-time/requirements.txt`
```
fastapi
uvicorn
mcp
httpx
python-jose[cryptography]
```

---

### Phase 2 – Integrate into docker-compose (1 AI-hour)

**Edit** (only once, in root):
```yaml
# docker-compose.yml
services:
  current-time:
    build: ./services/current-time
    restart: unless-stopped
    environment:
      - CONUSAI_PLATFORM_URL=http://agent-gateway:8080
      - CONUSAI_PLATFORM_TOKEN=${PLATFORM_ADMIN_TOKEN}
      - CONUSAI_TENANT_SCOPE=global                  # or "tenant-abc,tenant-xyz"
    ports:
      - "8082:8082"
    profiles: [full]
    depends_on:
      agent-gateway:
        condition: service_healthy
    command: ["sh", "-c", "python main.py"]
```

---

### Phase 3 – Tenant Scoping Logic (already handled by manifest — 0 extra AI-hours)

The `tenant_scope` field in the manifest is stored in `capability_embeddings.metadata`.  
`CapabilityRouter` (already in v0.3.2) reads it and filters per `TenantContext`.

**Future admin override** (no code change needed):
```bash
curl -X PATCH http://localhost:8080/admin/capabilities/current-time/enabled \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"tenant_ids": ["tenant-123"], "global": false}'
```

---

### Phase 4 – Test & Validation (2 AI-hours)

**Commands**:
```bash
# 1. Start everything
make db-reset
docker compose --profile full up -d

# 2. Verify registration
curl -H "Authorization: Bearer $PLATFORM_ADMIN_TOKEN" \
  http://localhost:8080/v1/capabilities/search?q=current+time

# 3. Test from agent (via chat/completions)
# Send a message that triggers the tool → should return timestamp
# Check workspace: file appears automatically via ArtifactBridge if you add an artifact later

# 4. Tenant test
# Set CONUSAI_TENANT_SCOPE=tenant-abc in compose → only that tenant sees it
```

**Expected**: Capability appears in semantic search, can be invoked, outputs land in workspace if artifacts are added.

---

### Phase 5 – How to Add Future Capabilities (True Zero-Core-Touch)

1. Copy `./services/current-time` → `./services/new-tool`.
2. Update `main.py` manifest + tools.
3. Add service block to `docker-compose.yml` (or deploy via Kubernetes/Helm with labels).
4. `docker compose up --build new-tool`.
5. Done — registers automatically, tenant-scoped, appears in workspace.

**No Rust, no Cargo, no rebuild of agent-gateway.**

---

### Effort & Token Summary

| Phase | AI-hours | Tokens     |
|-------|----------|------------|
| 0     | 1        | ~8k       |
| 1     | 3–4      | ~25k      |
| 2     | 1        | ~5k       |
| 3–4   | 2        | ~12k      |
| **Total** | **7–8** | **~50k** |

**Implementation effort for AI coder**: 7–8 AI-hours.  
**Risk**: None — fully leverages existing registrar, router, and bridge.

**Next after merge**:
- Tag `v0.3.3`.
- Update `docs/project-instructions.md` with “Zero-Core-Touch Pattern” section.
- Ship financial-services pack as the first real vertical using the same pattern (no core changes).

**Ready to implement?**  
Copy this entire Markdown into your AI coding agent and say **“implement this plan”**.  
The platform is now a true self-assembling agent system — exactly the Claude-beating architecture we designed.

Let’s ship zero-core-touch today. 🚀