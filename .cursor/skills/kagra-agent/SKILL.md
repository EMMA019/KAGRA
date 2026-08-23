---
name: kagra-agent
description: >-
  Build and verify KAGRA games/scenes with the engine's agent loop.
  Use when editing kagra examples, VRM scenes, verify scenarios, or when the
  user mentions KAGRA, Orb Rush, VRM, or maturin/wgpu rendering.
---

# KAGRA Agent Loop

## Rules

1. **Do not invent APIs.** Search `docs/API_INDEX.md` or MCP `kagra_api_search` first.
2. **Resolve assets via contracts.** Prefer `kagra.contracts.resolve_asset` / MCP `kagra_resolve_asset` (aliases: Emma, walk, dance).
3. **Close the loop.** After visual changes, run a verify scenario or MCP `kagra_render` / `kagra_verify`.
4. **One EventLoop per process on Windows.** Always run GPU scenes in a **subprocess** (`kagra.verify`).

## Commands

```bash
python tools/gen_api_index.py
python -m kagra.verify examples/verify_scenarios/blank_smoke.json
python tools/mcp_kagra/server.py   # MCP stdio
```

## MCP tools

- `kagra_api_search` – public signatures
- `kagra_env` – available VRM/FBX/BVH
- `kagra_resolve_asset` – kind + name → path
- `kagra_verify` – scenario JSON
- `kagra_render` – clear-color smoke screenshot

Short 3D: `Prop` / `Walk` / `sky()`. Texture via `texture_from_fn` / `load`.
Parent is 1 level (`set_parent`). Not 2D `Entity`.

## Touch / mobile

Use `kagra.touch.VirtualPad` + `PointerEvent` (`docs/schemas/input_events.json`).
`kagra-shared` + `mobile/` is a separate driving demo, not the Python game stack.
