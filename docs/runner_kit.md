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
  "style": {"sky": [10, 3, 4, 255], "edge_a": [255, 150, 40, 255], "edge_b": [170, 80, 255, 255]},
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

## Layers

| Layer | Python | Rust | Holds |
|---|---|---|---|
| Core | `kagra/game_spec.py` `GameSpec` | `game_spec.rs` `GameSpec` | `kind`, id/title, `seed`, `player`, entity tables (`obstacles`: glb, scale, hitbox, weight; deterministic weighted pick), `hit_inset`/`hit_top`, `camera` presets, neon `style`, `hud_top`, reserved `items`/`placements`, loading and validation, GLB hitbox measuring |
| Runner kit | `kagra/runner_kit.py` `RunnerKit` | `RunnerKit` (`spec.runner`) | `speed` curve, `jump`, `lanes`, the forward `obstacle_track` and `rings.track`, `gates`, spawn-ahead `draw` distances and fog |
| Runner shell | `kagra/runner.py` `play` / `main` / `RunnerScene` | `neon_cat.rs` sim | the window, headless, shots and bake modes; the fixed-dt sim, collision, juice (hitstop, shake, sparks, squash) and the score/HUD/result flow |

`kind` picks the kit. Today the only kind is `"runner"`. A new kit adds one
section next to `runner` and a new `GameKind` variant; the core does not
change.

Step 1 scope: the core is a split of the data. The sim code (collision,
juice, flow) still lives in `neon_cat.rs`. Step 2 should move it into a
shared module behind a small `Kit` trait (`tick`, `spawn`, `camera_target`).

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
