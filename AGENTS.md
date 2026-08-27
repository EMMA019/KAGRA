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
# shared wgpu 30 offscreen of a World.dump JSON (skips if no helper; not RendererV2)
python -m kagra.render_world scratch/orb_rush_world.json scratch/orb_rush_shared.png
# cargo run -p kagra-shared --features render --example offscreen -- W H out.png world dump.json
# real desktop window (wgpu 30; WASD + look; skips without a display; not RendererV2)
python -m kagra.play_world kagra-shared/tests/fixtures/crest_isle_world.json --seconds 8
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

- `kagra.texture_from_fn` / `kagra.tone` / `kagra.sound` — procedural art and SE
- `kagra.set_listener` / `play_se(..., x=, y=, z=)` / `play_loop` — 3D SE (distance + stereo pan). `sound()` stays 2D
- `set_point_light` / `set_spot_light` — 4 slots (`slot=0..3`). Slot 0 is the key
- `Camera3D.world_to_screen` — world → HUD pixels
- `avatar.set_position` / `avatar.set_yaw` — move a VRM in the arena
- `kagra.billboard_mesh` / `disk_mesh` / `quad_y_mesh` / `box_mesh` — 3D sprites / floor / crates
- `kagra.upload_mesh_3d` / `draw_mesh_id` — retain a mesh, draw by id
- `World` (`World3D` と同じ型) + `Camera3D.follow` — floor / box collision and a chase camera. `world.query` / `world.dump` / `world.load` read the world without a screenshot
- `Prop` / `Walk` / `sky()` / `room()` / `water()` — short 3D.
  Outdoor island: `World.set_height_fn(..., tile=, stream_radius=)` /
  `load_city` / `overworld_height` / `Walk(..., jump=)` /
  `Walk.wish` / `CharacterController` /
  `apply_outdoor_look()`. City JSON is not OSM. Dynamic boxes fall and
  stack; `Walk` stands on them (`add_box(..., is_static=False)`). Rapier
  crate stays out of the 5MB wheel. OSM is outside 80%.
- `Walk(first_person=True)` / `hovered_prop(cam)` / `clicked_prop(cam)` /
  `Walk.carry` — lock on first person, click to use, pick up
- `Walk.wish` / `Walk.move` / `Walk.try_jump` (or `CharacterController`) —
  accel/decel, slope sit, step-up, jump+land. Not Rapier. Sticky-walk quiet
  gap 3 is input.
- `animate` / `Label` / `Button` — tween and screen HUD
- `destroy(prop)` / `Prop.update_all(dt)` — kinematic move and delete
- Sphere / cylinder `Prop` collide and hover as those shapes (not boxes)
- `Prop(..., texture=…)` / `set_parent` — 4-level parent
- `Prop("crate.glb")` — static glTF part (not `stage()`). Alias `cube.glb`
- `axis("left")` / `pad("a")` / `inject_pad` — gamepad. `Walk` reads both sticks
- `kagra.save_json` / `load_json` — high scores
- `kagra.annotate(sx, sy)` — preview click → JSONL (screen / world / bone / Prop id). How 「ここもう少し」 becomes numbers. Not a visual editor
- `kagra.debug_trace(foot_y=…, height_fn=…)` — slope-float detector (`|foot-terrain|` while grounded). Default threshold 0.05. `debug_trace_summary()` → `frames 32-48 floated 0.15`. `World3D.update` feeds it when a tracer is active. Slope sit uses a tight foot AABB + 8-point ring + snap-to-plane, not Rapier.
- `Camera3D.follow(..., world=)` — pull the chase camera in so it does not go through walls
- `ActionController` — one-shot poses; `ActionController.names()` lists them
- `avatar.set_locomotion(speed)` — idle/walk/run speed blend (no clip snap).
  Local Mixamo Idle/Walk/Run: `avatar.bind_locomotion()` (rest+roll
  compensation onto VRoid). Do not resolve the `walk` alias
  (`synthetic_walk.bvh`). `play_upper` keeps spine/arms independent;
  ActionController overlays do not fight the walk arm swing.
  `dance()` is a full-body clip, not locomotion.
- `kagra.avatar(path)` twice shares GPU mesh/texture/MToon. Measure with
  `vrm_gpu_stats()`. Extra bodies: `examples/vrm_multi_avatar.py`
  (Crest Isle stays one player)

Verify: `examples/verify_scenarios/orb_rush_smoke.json`,
`heart_catch_smoke.json`, `switch_room_smoke.json`,
`dodge_room_smoke.json`, `prop_garden_smoke.json`,
and `pretty_room_smoke.json`, `overworld_smoke.json`,
`multi_avatar_smoke.json`.
Logged builds live in `docs/agent-runs/`.
The API index front is VRM / 3D / agents; the shelf is legacy 2D.

## More context

- `docs/AGENT.md` — contracts table, CI-parity commands, Cargo.lock policy
- `docs/API_INDEX.md` — the searchable public API
- `docs/REVIEW.ja.md` — engine review vs three.js / three-vrm / Ursina
- `docs/ROADMAP.ja.md` — 100% = 画面なしでインディーを出荷。80% はそのマイナス（ネット・破壊・布・乗り物・GI bake・DOTS・HDRP・人間エディタ・VRM-on-Wasm ほか）。今約 15%、山は看板(#97) → 世界をデータに → ランタイム一つ → ゲームとして足りる → 出荷。旧 63% は `docs/archive/`。頭脳は `kagra.brain("kairi")`
- `docs/schemas/input_events.json` — touch / pointer input schema
