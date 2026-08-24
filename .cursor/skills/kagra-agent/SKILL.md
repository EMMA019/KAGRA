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

Short 3D: `Prop` / `Walk` / `sky()` / `room()` / `water()`. Texture via `texture_from_fn` / `load`.
Parent is 2 levels (`set_parent`, grandchild OK). glTF parts: `Prop("crate.glb")` (not `stage()`).
Gamepad: `axis` / `pad` / `inject_pad`. USB/XInput via gilrs on the EventLoop.
`inject_pad` wins in tests. Not 2D `Entity`.
Picture: `set_point_light` / `set_spot_light` / `set_hdri("studio")` /
`set_exposure` / `set_tonemap` / `Prop(..., metallic=, normal=)`. Indoor: `apply_room_look`.
Outdoor: `apply_outdoor_look` / `World3D.set_height_fn(overworld_height, tile=10, stream_radius=28)` /
`load_city` / `Walk(..., jump=)`. City JSON is not OSM; physics is not Rapier yet.
Play: `clicked_prop` / `Walk.carry` / `animate` / `Label` / `sound`.
Pointer lock follows first person (OS may refuse). USB pad is gilrs on the
EventLoop (`inject_pad` still wins for CI).

Current engine bar is the usable week in `docs/ROADMAP.ja.md` (pixels;
normals / USB pad APIs landed). Indoor spot shadows the local light, not
the sun. Pairwise goldens: `indoor_spot` / `tonemap_on` / `ibl_metal` (CI
`golden` job). IBL metal look and outdoor crawl pixels still open. Final
goal is first-recall, not this bar. Brain hook is `kagra.brain("kairi")` — default `https://kairi.onrender.com`,
token in `KAIRI_API_TOKEN`. Not in the wheel. Later engine (Rapier / OSM /
more CSM) is deferred, not banned. Do not start D-6 as a fourth box room;
D-6 needs 30s of play and a score or goal. Do not call APIs three.js-class
until the 30s demo test.

## Touch / mobile

Use `kagra.touch.VirtualPad` + `PointerEvent` (`docs/schemas/input_events.json`).
`kagra-shared` + `mobile/` is a separate driving demo, not the Python game stack.
