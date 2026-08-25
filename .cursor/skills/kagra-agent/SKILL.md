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
Parent is 4 levels (`set_parent`). glTF parts: `Prop("crate.glb")` (not `stage()`).
Gamepad: `axis` / `pad` / `inject_pad`. USB/XInput via gilrs on the EventLoop.
`inject_pad` wins in tests. Not 2D `Entity`.
Picture: `set_point_light` / `set_spot_light` (`slot=0..3`; 0 is the key) /
`set_hdri("studio")` /
`set_exposure` / `set_tonemap` / `Prop(..., metallic=, normal=)`. Indoor: `apply_room_look`.
Outdoor: `apply_outdoor_look` / `World3D.set_height_fn(overworld_height, tile=10, stream_radius=28)` /
`load_city` / `Walk(..., jump=)`. City JSON is not OSM. Dynamic boxes
fall and stack; `Walk` stands on them (`add_box(..., is_static=False)`).
Play: `clicked_prop` / `Walk.carry` / `animate` / `Label` / `sound`.
3D SE: `set_listener` / `play_se(..., x=, y=, z=)` / `play_loop` (distance + stereo pan).
`avatar.set_locomotion(speed)` blends idle/walk/run. Local Mixamo FBX:
`avatar.bind_locomotion()` (rest+roll onto VRoid; never the `walk` alias).
`play_upper` / `ActionController` overlay spine/arms without fighting the legs.
Same-path `kagra.avatar()` shares GPU mesh/texture/MToon (`vrm_gpu_stats()`).
Agent eyes: `kagra.annotate` (click → JSONL) / `kagra.debug_trace` (foot vs terrain, threshold 0.05).
Slope sit is a tight foot AABB + snap-to-plane; still no Rapier. Not a Tk/Inspector.
Pointer lock follows first person (OS may refuse). USB pad is gilrs on the
EventLoop (`inject_pad` still wins for CI).

Engine target is **80%** in `docs/ROADMAP.ja.md` (now ~63%). 100% = Python
replacement of three-vrm + three.js + Ursina for everyday work. Body is
~80%. Picture is ~85% (indoor, crawl, normals, local-four, ACES/metal on CI).
Parent is 4 levels. Local lights are `slot=0..3`.
Rigid boxes are AABB equal (fall / stack / stand). Next: the 30s demos.
OSM / 4-cascade CSM / SSAO stay outside 80%.
Final goal is first-recall; 80% is not a substitute. Brain hook is
`kagra.brain("kairi")` — default `https://kairi.onrender.com`, token in
`KAIRI_API_TOKEN`. Not in the wheel. Do not start D-6 as a fourth box room;
D-6 needs 30s of play and a score or goal. Do not call 80% or three.js-class
until pixels and the 30s demos say so.

## Touch / mobile

Use `kagra.touch.VirtualPad` + `PointerEvent` (`docs/schemas/input_events.json`).
`kagra-shared` + `mobile/` is a separate driving demo, not the Python game stack.
