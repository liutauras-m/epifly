"""
current-time MCP capability service.
Zero-core-touch: self-registers at startup via POST /admin/capabilities/register.
Exposes a single tool: get_current_time(timezone?) → ISO 8601 timestamp.

Implements the JSON-RPC 2.0 MCP wire protocol over plain HTTP POST so it
is compatible with ConusAI's McpAdapter (which posts JSON-RPC to the endpoint).
"""
import asyncio
import os
from datetime import datetime
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

import httpx
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse


app = FastAPI(title="current-time MCP service")


# ── JSON-RPC MCP endpoint ──────────────────────────────────────────────────────

TOOL_DEF = {
    "name": "get_current_time",
    "description": "Returns ISO 8601 timestamp for a given IANA timezone (default UTC).",
    "inputSchema": {
        "type": "object",
        "properties": {
            "timezone": {
                "type": "string",
                "description": "IANA timezone name, e.g. 'Europe/Helsinki' or 'America/New_York'",
            }
        },
    },
}


async def _get_current_time(timezone: str = "UTC") -> dict:
    """Returns the current ISO 8601 timestamp. Optional IANA timezone name."""
    try:
        tz = ZoneInfo(timezone)
    except ZoneInfoNotFoundError:
        return {
            "content": f"Error: Unknown timezone '{timezone}'",
            "artifacts": [],
            "metadata": {},
        }
    now = datetime.now(tz)
    return {
        "content": f"Current time in {timezone}: {now.isoformat()}",
        "artifacts": [],
        "metadata": {"timezone": timezone, "timestamp": now.isoformat()},
    }


@app.post("/mcp")
async def mcp_handler(request: Request):
    """Handle MCP JSON-RPC 2.0 requests."""
    try:
        body = await request.json()
    except Exception:
        return JSONResponse(
            status_code=400,
            content={"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Parse error"}},
        )

    req_id = body.get("id")
    method = body.get("method", "")
    params = body.get("params") or {}

    if method == "tools/list":
        return {"jsonrpc": "2.0", "id": req_id, "result": {"tools": [TOOL_DEF]}}

    if method == "tools/call":
        name = params.get("name")
        args = params.get("arguments") or {}
        if name == "get_current_time":
            result = await _get_current_time(**{k: v for k, v in args.items()})
            return {"jsonrpc": "2.0", "id": req_id, "result": result}
        return {
            "jsonrpc": "2.0",
            "id": req_id,
            "error": {"code": -32601, "message": f"Unknown tool: {name}"},
        }

    return {
        "jsonrpc": "2.0",
        "id": req_id,
        "error": {"code": -32601, "message": f"Method not found: {method}"},
    }


# ── Self-registration ──────────────────────────────────────────────────────────

MANIFEST = {
    "capability_id": "media.time.current-time",
    "name": "current-time",
    "namespace": "media.time",
    "description": "Returns current server time with optional IANA timezone support.",
    "version": "1.0.0",
    "kind": "remote_mcp",
    "endpoint": (os.getenv("SERVICE_URL") or os.getenv("CONUSAI_SERVICE_URL", "http://current-time:8082")) + "/mcp",
    "tools": [
        {
            "name": "get_current_time",
            "description": "Returns ISO 8601 timestamp for a given IANA timezone (default UTC).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "timezone": {
                        "type": "string",
                        "description": "IANA timezone name, e.g. 'Europe/Helsinki' or 'America/New_York'",
                    }
                },
            },
        }
    ],
    "tenant_scope": (
        [t for t in os.getenv("CONUSAI_TENANT_SCOPE", "").split(",") if t]
        if os.getenv("CONUSAI_TENANT_SCOPE")
        else []
    ),
    "enabled": True,
    "tags": ["time", "utility"],
}


async def register_with_retry(max_retries: int = 10, delay: float = 3.0) -> None:
    # Canonical names: GATEWAY_URL / PLATFORM_ADMIN_TOKEN.
    # CONUSAI_PLATFORM_URL / CONUSAI_PLATFORM_TOKEN kept as one-release fallbacks.
    _conusai_url = os.environ.get("CONUSAI_PLATFORM_URL")
    _conusai_token = os.environ.get("CONUSAI_PLATFORM_TOKEN")
    if _conusai_url and not os.environ.get("GATEWAY_URL"):
        print("[current-time] CONUSAI_PLATFORM_URL is deprecated; use GATEWAY_URL", flush=True)
    if _conusai_token and not os.environ.get("PLATFORM_ADMIN_TOKEN"):
        print("[current-time] CONUSAI_PLATFORM_TOKEN is deprecated; use PLATFORM_ADMIN_TOKEN", flush=True)
    platform_url = os.environ.get("GATEWAY_URL") or _conusai_url or "http://agent-gateway:8080"
    token = os.environ.get("PLATFORM_ADMIN_TOKEN") or _conusai_token or ""
    url = f"{platform_url}/admin/capabilities/register"

    headers = {}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    # In dev mode (no JWT_SECRET), X-Tenant-ID header is accepted instead.
    else:
        headers["X-Tenant-ID"] = "dev"

    for attempt in range(1, max_retries + 1):
        try:
            async with httpx.AsyncClient(timeout=10) as client:
                resp = await client.post(url, json=MANIFEST, headers=headers)
                resp.raise_for_status()
                print(f"[current-time] registered: {resp.json()}", flush=True)
                return
        except Exception as exc:
            print(
                f"[current-time] registration attempt {attempt}/{max_retries} failed: {exc}",
                flush=True,
            )
            if attempt < max_retries:
                await asyncio.sleep(delay)

    print(
        "[current-time] WARNING: all registration attempts failed — service running unregistered",
        flush=True,
    )


@app.on_event("startup")
async def on_startup() -> None:
    asyncio.create_task(register_with_retry())
