# AGENTS.md

Instructions for AI coding agents working in this repository
(Claude Code, Cursor, Windsurf, Codex, aider, ...). Cursor users get the
same rules automatically via `.cursor/skills/kagra-agent/SKILL.md`.

KAGRA's development loop is designed so that an agent can build and verify
a game **without a human looking at the screen**.

## Rules

1. **Do not invent APIs.** Search `docs/API_INDEX.md` (or MCP
   `kagra_api_search`) first. The index is generated from the AST:
   `python tools/gen_api_index.py --check` must stay clean.
2. **Resolve assets via contracts.** Use `kagra.contracts.resolve_asset`
   (or MCP `kagra_resolve_asset`). Aliases: `Emma`, `walk`, `dance`.
3. **Close the loop.** After any visual change, run a verify scenario or
   MCP `kagra_render` / `kagra_verify`. Never claim "done" without it.
4. **One EventLoop per process on Windows.** Always run GPU scenes in a
   subprocess — that is what `kagra.verify` does for you.
5. **Keep tests extension-free.** `tests/` must not import the Rust
   extension directly; load pure-logic modules via
   `tests/conftest.py::load_kagra_submodule`. Run
   `pytest tests -m "not golden"`.
6. **Log build sessions.** When you build a game or scene on request,
   save the prompt, the key decisions, and the verify results under
   `docs/agent-runs/` (see `docs/agent-runs/README.md`). The log is a
   first-class artifact, not an afterthought.

## Commands

```bash
python tools/gen_api_index.py --check                              # API index drift
python -m kagra.verify examples/verify_scenarios/blank_smoke.json  # headless smoke
python -m kagra.verify examples/verify_scenarios/orb_rush_smoke.json
python tools/mcp_kagra/server.py                                   # MCP (stdio)
pytest tests -m "not golden"                                       # pure-python tests
```

## MCP tools (`tools/mcp_kagra/server.py`)

| Tool | What it does |
|---|---|
| `kagra_api_search` | search public signatures |
| `kagra_env` | list available VRM / FBX / BVH assets |
| `kagra_resolve_asset` | kind + name → path |
| `kagra_verify` | run a scenario JSON headlessly |
| `kagra_render` | clear-color smoke screenshot |

## Reference game

`examples/vrm_orb_rush.py` is the reference game for this loop (title →
countdown → play → result, procedural SFX, particles, difficulty curve)
and is written against **public APIs only**. Prefer these over hand-rolled
PNG/WAV/projection:

- `kagra.texture_from_fn` / `kagra.tone` — procedural art and SE
- `Camera3D.world_to_screen` — world → HUD pixels
- `avatar.set_position` / `avatar.set_yaw` — move a VRM in the arena
- `kagra.billboard_mesh` / `disk_mesh` / `quad_y_mesh` / `box_mesh` — 3D sprites / floor / crates
- `kagra.upload_mesh_3d` / `draw_mesh_id` — retain a mesh, draw by id
- `World3D` + `Camera3D.follow` — floor / box collision and a chase camera
- `Prop` / `Walk` / `sky()` / `room()` / `water()` — short 3D.
  Outdoor island: `World3D.set_height_fn(..., tile=, stream_radius=)` /
  `load_city` / `overworld_height` / `Walk(..., jump=)`.
  `set_shadow_cascades(2)` outdoors. Not OSM / Rapier.
- `Walk(first_person=True)` / `hovered_prop(cam)` — eye-height view and mouse pick
- `destroy(prop)` / `Prop.update_all(dt)` — kinematic move and delete
- Sphere / cylinder `Prop` collide and hover as those shapes (not boxes)
- `Prop(..., texture=…)` / `set_parent` — 1-level parent only; child pose is local
- `Prop("crate.glb")` — static glTF part (not `stage()`). Alias `cube.glb`
- `axis("left")` / `pad("a")` / `inject_pad` — gamepad. `Walk` reads both sticks
- `kagra.save_json` / `load_json` — high scores
- `ActionController` — one-shot poses; `ActionController.names()` lists them

Verify: `examples/verify_scenarios/orb_rush_smoke.json`,
`heart_catch_smoke.json`, `switch_room_smoke.json`,
`dodge_room_smoke.json`, `prop_garden_smoke.json`,
and `pretty_room_smoke.json`, `overworld_smoke.json`.
Logged builds live in `docs/agent-runs/`.
The API index front is VRM / 3D / agents; the shelf is legacy 2D.

## More context

- `docs/AGENT.md` — contracts table, CI-parity commands, Cargo.lock policy
- `docs/API_INDEX.md` — the searchable public API
- `docs/REVIEW.ja.md` — engine review vs three.js / three-vrm / Ursina
- `docs/ROADMAP.ja.md` — demand wedges + capability tracks
- `docs/schemas/input_events.json` — touch / pointer input schema
