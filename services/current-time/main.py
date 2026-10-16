"""
current-time MCP capability service.
Zero-core-touch: self-registers at startup via POST /admin/capabilities/register.
Exposes a single tool: get_current_time(timezone?) → ISO 8601 timestamp.

Implements the JSON-RPC 2.0 MCP wire protocol over plain HTTP POST so it
is compatible with ConusAI's McpAdapter (which posts JSON-RPC to the endpoint).
"""
import asyncio
import os
from contextlib import asynccontextmanager
from datetime import datetime
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

import httpx
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse


@asynccontextmanager
async def lifespan(_app: "FastAPI"):
    task = asyncio.create_task(register_with_retry())
    yield
    task.cancel()


app = FastAPI(title="current-time MCP service", lifespan=lifespan)


# ── JSON-RPC MCP endpoint ──────────────────────────────────────────────────────

TOOL_DEF = {
    "name": "get_current_time",
    "description": "Get the current time, date, or clock reading. Returns ISO 8601 timestamp for a given IANA timezone (default UTC). Use this when asked: what time is it, what is the current time, what is today's date, now, current datetime.",
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


@app.get("/health")
async def health() -> dict:
    return {"status": "ok"}


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
    "namespace": "media.time",
    "version": "1.0.0",
    "kind": "remote_mcp",
    "endpoint": os.environ["SERVICE_URL"] + "/mcp",
    "tools": [
        {
            "name": "get_current_time",
            "description": "Get the current time, date, or clock reading. Returns ISO 8601 timestamp for a given IANA timezone (default UTC). Use this when asked: what time is it, what is the current time, what is today's date, now, current datetime.",
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
    # A platform utility, visible to every tenant (the gateway requires the explicit opt-in).
    "global": True,
    "enabled": True,
    "tags": ["time", "utility"],
}


async def register_with_retry(max_retries: int = 25, delay: float = 5.0) -> None:
    url = f"{os.environ['GATEWAY_URL']}/admin/capabilities/register"
    headers = {"Authorization": f"Bearer {os.environ['PLATFORM_ADMIN_TOKEN']}"}

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
