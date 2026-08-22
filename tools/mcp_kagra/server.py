#!/usr/bin/env python3
"""KAGRA MCP server（エージェント閉ループ）。

Tools:
  - kagra_api_search: 公開 API 索引を検索
  - kagra_env: アセット環境スナップショット
  - kagra_resolve_asset: VRM/FBX 等のパス解決
  - kagra_verify: シナリオ JSON を実行してスクショ等を検証
  - kagra_render: インライン短時間レンダー（スクショ）

Cursor 登録例 (.cursor/mcp.json)::

    {
      "mcpServers": {
        "kagra": {
          "command": "python",
          "args": ["tools/mcp_kagra/server.py"],
          "cwd": "${workspaceFolder}"
        }
      }
    }

依存: 可能なら `pip install mcp`。無い場合は最小 JSON-RPC stdio 実装にフォールバック。
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))


def _api_search(query: str, limit: int = 20) -> str:
    import importlib.util

    gen = ROOT / "tools" / "gen_api_index.py"
    spec = importlib.util.spec_from_file_location("kagra_gen_api_index", gen)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    q = (query or "").strip().lower()
    items = mod._runtime_enrich(mod._public_from_init())
    if q:
        items = [i for i in items if q in i[0].lower() or q in i[1].lower()]
    items = items[: max(1, min(limit, 100))]
    lines = [f"{name}: {sig}" for name, sig, _kind in items]
    return "\n".join(lines) if lines else f"(no matches for {query!r})"


def _env() -> str:
    from kagra.contracts import dump_environment_json

    return dump_environment_json()


def _resolve(kind: str, name: str) -> str:
    from kagra.contracts import AssetKind, KagraContractError, resolve_asset

    try:
        k = AssetKind(kind.lower())
    except ValueError:
        return json.dumps(
            {
                "ok": False,
                "error": {
                    "code": "BAD_KIND",
                    "message": f"unknown kind {kind}",
                    "hint": "use vrm|fbx|bvh|vrma|gltf|texture|font|audio|any",
                },
            },
            ensure_ascii=False,
        )
    try:
        path = resolve_asset(k, name)
        return json.dumps({"ok": True, "path": str(path)}, ensure_ascii=False)
    except KagraContractError as e:
        return json.dumps({"ok": False, "error": e.to_dict()}, ensure_ascii=False)


def _verify(scenario_path: str = "", scenario_json: str = "") -> str:
    from kagra.verify import VerifyResult, _load_scenario, run_scenario, run_scenario_path

    if scenario_json.strip():
        data = json.loads(scenario_json)
        result = run_scenario(_load_scenario(data))
    elif scenario_path.strip():
        result = run_scenario_path(scenario_path)
    else:
        result = VerifyResult(
            ok=False, name="?", elapsed_sec=0.0, error="need scenario_path or scenario_json"
        )
    return json.dumps(result.to_dict(), ensure_ascii=False, indent=2)


def _render(
    out: str = "scratch/mcp_render.png",
    width: int = 320,
    height: int = 180,
    frames: int = 10,
) -> str:
    from kagra.verify import Scenario, run_scenario

    sc = Scenario(
        name="mcp_render",
        inline={
            "width": width,
            "height": height,
            "max_frames": frames,
            "screenshot_at": max(1, frames // 2),
            "out": out,
        },
        timeout_sec=90,
    )
    result = run_scenario(sc)
    return json.dumps(result.to_dict(), ensure_ascii=False, indent=2)


def _run_fastmcp() -> None:
    from mcp.server.fastmcp import FastMCP

    mcp = FastMCP(
        "kagra",
        instructions=(
            "KAGRA game engine tools. Prefer kagra_api_search before inventing APIs. "
            "Use kagra_verify / kagra_render to close the write→run→screenshot loop."
        ),
    )

    @mcp.tool()
    def kagra_api_search(query: str = "", limit: int = 20) -> str:
        """Search KAGRA public Python API names and signatures."""
        return _api_search(query, limit)

    @mcp.tool()
    def kagra_env() -> str:
        """List available VRM/FBX/BVH assets and aliases for this checkout."""
        return _env()

    @mcp.tool()
    def kagra_resolve_asset(kind: str, name: str) -> str:
        """Resolve an asset logical name (e.g. kind=vrm name=Emma) to a filesystem path."""
        return _resolve(kind, name)

    @mcp.tool()
    def kagra_verify(scenario_path: str = "", scenario_json: str = "") -> str:
        """Run a verify scenario (JSON file path or inline JSON string). Returns structured result."""
        return _verify(scenario_path, scenario_json)

    @mcp.tool()
    def kagra_render(
        out: str = "scratch/mcp_render.png",
        width: int = 320,
        height: int = 180,
        frames: int = 10,
    ) -> str:
        """Headless clear-color render + screenshot. Smoke-test that GPU path works."""
        return _render(out, width, height, frames)

    mcp.run(transport="stdio")


# ── 最小 stdio JSON-RPC（mcp パッケージ無し時） ───────────────

def _read_message() -> dict | None:
    """Content-Length framing."""
    headers: dict[str, str] = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line in (b"\r\n", b"\n"):
            break
        if b":" in line:
            k, v = line.decode("utf-8").split(":", 1)
            headers[k.strip().lower()] = v.strip()
    length = int(headers.get("content-length", "0"))
    if length <= 0:
        return None
    body = sys.stdin.buffer.read(length)
    return json.loads(body.decode("utf-8"))


def _write_message(msg: dict) -> None:
    raw = json.dumps(msg, ensure_ascii=False).encode("utf-8")
    sys.stdout.buffer.write(f"Content-Length: {len(raw)}\r\n\r\n".encode("ascii"))
    sys.stdout.buffer.write(raw)
    sys.stdout.buffer.flush()


def _run_minimal() -> None:
    tools = [
        {
            "name": "kagra_api_search",
            "description": "Search KAGRA public Python API",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer"},
                },
            },
        },
        {
            "name": "kagra_env",
            "description": "Asset environment snapshot",
            "inputSchema": {"type": "object", "properties": {}},
        },
        {
            "name": "kagra_resolve_asset",
            "description": "Resolve asset path",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": {"type": "string"},
                    "name": {"type": "string"},
                },
                "required": ["kind", "name"],
            },
        },
        {
            "name": "kagra_verify",
            "description": "Run verify scenario",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scenario_path": {"type": "string"},
                    "scenario_json": {"type": "string"},
                },
            },
        },
        {
            "name": "kagra_render",
            "description": "Headless smoke render",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "out": {"type": "string"},
                    "width": {"type": "integer"},
                    "height": {"type": "integer"},
                    "frames": {"type": "integer"},
                },
            },
        },
    ]

    while True:
        msg = _read_message()
        if msg is None:
            break
        mid = msg.get("id")
        method = msg.get("method")
        params = msg.get("params") or {}

        if method == "initialize":
            _write_message(
                {
                    "jsonrpc": "2.0",
                    "id": mid,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "kagra", "version": "0.1.0"},
                    },
                }
            )
        elif method == "notifications/initialized":
            continue
        elif method == "tools/list":
            _write_message({"jsonrpc": "2.0", "id": mid, "result": {"tools": tools}})
        elif method == "tools/call":
            name = params.get("name")
            args = params.get("arguments") or {}
            try:
                if name == "kagra_api_search":
                    text = _api_search(args.get("query", ""), int(args.get("limit", 20)))
                elif name == "kagra_env":
                    text = _env()
                elif name == "kagra_resolve_asset":
                    text = _resolve(args["kind"], args["name"])
                elif name == "kagra_verify":
                    text = _verify(args.get("scenario_path", ""), args.get("scenario_json", ""))
                elif name == "kagra_render":
                    text = _render(
                        args.get("out", "scratch/mcp_render.png"),
                        int(args.get("width", 320)),
                        int(args.get("height", 180)),
                        int(args.get("frames", 10)),
                    )
                else:
                    text = f"unknown tool: {name}"
                _write_message(
                    {
                        "jsonrpc": "2.0",
                        "id": mid,
                        "result": {"content": [{"type": "text", "text": text}]},
                    }
                )
            except Exception as e:
                _write_message(
                    {
                        "jsonrpc": "2.0",
                        "id": mid,
                        "result": {
                            "content": [{"type": "text", "text": f"error: {e}"}],
                            "isError": True,
                        },
                    }
                )
        elif method == "ping":
            _write_message({"jsonrpc": "2.0", "id": mid, "result": {}})
        else:
            if mid is not None:
                _write_message(
                    {
                        "jsonrpc": "2.0",
                        "id": mid,
                        "error": {"code": -32601, "message": f"Method not found: {method}"},
                    }
                )


def main() -> None:
    try:
        import mcp  # noqa: F401

        _run_fastmcp()
    except ImportError:
        _run_minimal()


if __name__ == "__main__":
    main()
