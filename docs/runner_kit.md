# Game spec and runner kit

A Neon Cat-class game is a short Python file plus a few lines of JSON. The
JSON holds only what differs from Neon Cat. Python authors the spec and the
Rust sim plays it. Rust is needed because wasm and Android have no Python.

```python
# examples/meteor_cat_minimal.py (9 lines with docstring and imports)
from kagra.runner import play
raise SystemExit(play("games/meteor_cat_minimal.json"))
```

```json
"game": {
  "id": "meteor_cat", "title": "METEOR CAT",
  "obstacles": ["METEOR", {"name": "CRYSTALS", "weight": 2}, "ROCK"],
  "style": {"sky": [10, 3, 4, 255], "codes": {"jump_over": [255, 150, 40]}},
  "runner": {"speed": {"start": 12, "cap": 24}}
}
```

This goes inside the usual `games/*.json` manifest, next to `world` and
`bundle_assets`. Neon Cat itself is `"game": {}`.

- `python examples/meteor_cat_minimal.py --bake` writes the full spec into the
  `world` JSON, which wasm and Android load.
- `--headless out.png` renders headless and `--shots DIR` writes screenshots.
  With no flag it opens a window.

Obstacles can be given three ways:

- a `CATALOG` name, which covers the bundled CC0 crate, barrel, barrier,
  meteor, crystals, rock, dish and turret
- `{name, weight}`
- a full row `{name, glb, scale}`; the hitbox is measured from the GLB

Both sides reject unknown keys, so a typo fails loudly.

## Behaviors, colour codes and the course

Each obstacle kind has a `behavior`: `jump_over` (default, the old crate rule), `roll_under`, `solid`,
`rideable` and `cat_gap`. A kind is a base mesh plus optional `parts` (a ramp, a lintel, wall sides), each with a role
(`solid`, `ride`, `ramp`, `decor`), an offset and a model-space `hit_y` clip, so hollow shapes (a beam over a gap, a roof you can stand on)
need no special engine code. A `decor` part with `glow > 0` is drawn emissive in the kind's colour (the lit opening of a cat gap). Default
`OVERPASS` is a sign on two posts with 0.5 m under it, never a block: the lens sits 2.35 m up, and the harness fails any
spec whose obstacle comes within `CAMERA_CLEAR` (0.3 m) of it. Cat body: `runner.body`
`{stand_height, roll_height, roll_secs, step_tol, recover_secs, crouch}`: one roll lasts `roll_secs` and a press while rolling is
ignored; the body then stands back up over `recover_secs` (hitbox and pose together); `crouch` is the pose scale at full crouch.
`input.reverse_guard_secs` drops an up flick that comes that soon after a down flick (the finger returning).
`runner.score` `{per_meter, per_gate}` is the score formula: `per_meter` x distance, `per_gate` for each stone arch passed,
plus `rings.score` and `coins.score` per pickup (the recorded legacy runs set `per_gate` to 0). `runner.coins` sets coin
lines (score only). `runner.sections[].allow` limits which behaviors a section may spawn.

`hud` (any game spec, not only runners) styles the in-play HUD and the game-over screen: `hud.scale`, `hud.panel`
`{fill, border, coin, score, slice}` (a 9-slice panel for the score and a coin chip with a drawn coin icon, sized from the screen
width so they are about 30 px tall numbers on a phone), and `hud.result`: `beat_secs` (the crash plays out with shake, a red flash
and dimmed HUD for this long before the result), `deaths` (obstacle name -> the line shown, with `death_fallback`), and
`hud.result.board` `{enabled, top_n, columns, sort}`: a local top-N list saved on the device (localStorage, key
`kagra.board.v1.<id>:<seed>`, no network). Columns come from `time`, `gates`, `coins`, `score`; the result card shows the player's
run, highlights it in the list, and a tab per column re-sorts. The sim reports `result` (key, config, this run's values) and
`death_text` in its state, so another kit gets the same card by filling the same fields.

`style.codes` is the colour language, one neon colour per thing to do, used for the obstacle edge and its glow:
`jump_over` amber (low, hop), `roll_under` pink (high or hanging), `solid` red (go round), `cat_gap` violet (roll through),
`rideable` blue (run on it), `pass_through` mint (gates and arches, never a threat). A kind may override with `color`.
`validate` rejects two codes that are too alike. Gameplay is by action (jump, roll, lane), not by name, so a skin can retheme the
palette and labels without touching mechanics. The default course is generated, not hand-placed: `GameSpec::legacy()` is the old
three-crate stream the golden tests replay.

Models: `assets/models/kenney/space-kit/` (Kenney Space Kit, CC0), fetched by `tools/fetch_assets` with its record.

## Layers

| Layer | Python | Rust | Holds |
|---|---|---|---|
| Core | `kagra/game_spec.py` `GameSpec` | `game_spec.rs` `GameSpec` | `kind`, id/title, `seed`, `player`, entity tables (`obstacles`: glb, scale, hitbox, weight; deterministic weighted pick), `hit_inset`/`hit_top`, `camera` presets, neon `style`, `hud_top`, reserved `items`/`placements`, loading and validation, GLB hitbox measuring |
| Runner kit | `kagra/runner_kit.py` `RunnerKit` | `RunnerKit` (`spec.runner`) | `speed` curve, `jump`, `lanes`, the forward `obstacle_track` and `rings.track`, `gates`, spawn-ahead `draw` distances and fog |
| Runner shell | `kagra/runner.py` `play` / `main` / `RunnerScene` | `neon_cat.rs` sim | the window, headless, shots and bake modes; the fixed-dt sim, collision, juice (hitstop, shake, sparks, squash) and the score/HUD/result flow |

`kind` picks the kit. Today the only kind is `"runner"`. A new kit adds one
section next to `runner` and a new `GameKind` variant; the core does not
change.

Shared sim code lives in `kagra-shared/src/game_core/`:

- `juice`: `Spring1`, `JuiceShake`, `Juice` (shake, hitstop, landing bump,
  seeded spark pool)
- `collide`: `Hitbox::place` / `feet_hit` from an entity row
- `run`: `RunStats` (score, pickups, death) and the HUD score panel

`game_core::tick_frame(kit, dt)` is the fixed-order frame driver. It
handles the dt clamp, substeps, hitstop and its hand-off to the result
screen, title attract, and spark and juice decay. A kit implements `Kit`,
which needs `substeps`, `step`, `attract`, `follow`, `blocks_spark` and
`rebuild`. `neon_cat.rs` is now only the runner kit: lanes, forward scroll,
spawn-ahead and drawing.

## A second kit: a top-down arena dodger

- Reuses from the core: player, entity tables and hitboxes, style, camera
  presets (a top-down preset), juice, HUD, result flow, seed, and items
  (`shield` and `slowmo` work unchanged).
- New `arena` section: arena radius or bounds, a spawn schedule (wave rate
  over time instead of a forward track), a homing or bouncing motion per
  entity, and 2-axis steering instead of lanes plus jump.
- New Rust: an `ArenaKit` sim (2D movement, spawn from the edges), a
  top-down camera rig, and `GameKind::Arena`. New Python: `ArenaKit`
  dataclasses and `kagra/arena.py` reusing the `RunnerScene` shell.

## Reserved, not simulated yet

- `items[]` {effect, glb, weight, duration}: `effect` must be one of
  `shield`, `magnet`, `slowmo`, `double` or `boost`. Both sides reject any
  other value.
- `placements[]` {z, lane, kind}: seeded chunks baked in Python. They will
  replace the sine track when the list is non-empty.

## Determinism

`golden_runs_match_the_pre_port_numbers` (`neon_cat.rs`) replays fixed inputs
on both the default spec and the baked Neon Cat world. It checks that score,
rings, death frame and death reason match the numbers recorded on master
before the port. `second_game_is_just_a_spec` checks that Meteor Cat loads and
replays exactly. `tests/test_runner_spec.py` checks that each baked world
equals its manifest overrides.
