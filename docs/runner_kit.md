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

`lives` `{count 3, invuln_secs 1.6, blink_hz 10}` is the three lives (set `count` up to 9 for more). A hit costs one, plays a short shake, spark burst and thin red
screen edges, and the cat blinks and cannot be hit again for `invuln_secs` (so a ramp or wall cannot take two lives in one touch).
The last life ends the run exactly as before, with the same crash beat. The HUD shows one paw per life under the score (the one
just lost pops red; a spec with `count` 1 draws no row). `count` is 1..=9. Hits are taken at the single crash point, so every
behavior is covered. The measurement harness always runs with one life (`NeonCat::one_life`), because reachability and fairness
need a hit to equal a death; `GameSpec::legacy()` and the classic fixture use one life too, so the golden runs are unchanged.
`player.rim` `{left, right, intensity, radius, side, height}` are two coloured point lights (screen-left magenta, screen-right
cyan) that sit just outside the cat's flanks so its side faces catch an edge; they must be outside the body, and the radius stays
under 1.6 m so they never paint the floor. `player.eyes` holds the cat's glow marks, each with its own switch (model units, so they
follow the scale and pose; all bloom above glow 1). The chase camera only sees the cat's back, so besides the two eyes
`{enabled, color, glow, size, spread, height, forward}` there are ear-tip glints `{ears, ear_x, ear_y, ear_z}` (cyan), a neon collar
`{collar, collar_color, collar_y}` (three thin strips across the back and flanks; off by default, it read as a white band) and a tail-tip glint
`{tail_tip, tail_color, tail_x, tail_y, tail_z}` (magenta), which read from behind. `player.rim.intensity` is 3.6 so the flanks catch more light.

`items` `[{effect, duration?, weight, color?, icon?}]` and `item_rules` `{rest_every 3, reward_every 2, hover, radius 1.1, magnet_radius 4,
magnet_lead 4, pull_speed 6, pull_accel 70, warn_secs 1.5, boost_speed, jump_scale}` are the pick-ups. `effect` is one of `magnet` (coins fly
in, 8 s), `boost` (1.5x speed and nothing hurts, 3.5 s), `superjump` (jump 1.5x as high, 10 s), `double` (score gain doubles, 10 s) and
`meow` (+1 life, or +100 score when full). Orbs sit only in free slots of Rest and Reward sections (every `rest_every` / `reward_every`
free slot, half a gap past the slot so they never touch the coin line), the kind picked by weight from a hash of the slot.

Each kind has its own look: a `color` (0,0,0 means the effect's own: magnet magenta, boost orange, superjump green, double lemon, meow
pale pink) and an `icon` (`horseshoe`, `chevron`, `arrow`, `two_x`, `heart`), built from primitives, bobbing and swaying in a spinning
ring of the same colour. The same colour is used by the toast, the label and bar under the paws, and the floor ring. Both sides refuse a
palette where two items, an item and the coin yellow (255,214,90), or an item and an obstacle code are closer than 90 in summed RGB
channel difference (the same rule as `style.codes`); override `color` per game when its codes are close (Meteor Cat makes boost cyan
because its jump_over is orange).

The magnet does not snap: a coin inside the field (a ball of `magnet_radius`, stretched `magnet_lead` metres ahead of the cat so coins
are taken before the cat reaches them) is taken hold of and flies to the cat starting at `pull_speed` m/s and gaining `pull_accel` m/s2,
with a short streak behind it. When it lands it sparks and a "+N" pop (summing the coins landing in a row) shows under the coin chip.
Two rings sweep in toward the cat inside the field's radius while the magnet is on. In the last `warn_secs` the label, bar, aura and
rings blink. `radius` (1.1 m) is the pick-up reach for orbs. `GameSpec::legacy()` and the classic fixture have no items or props,
so the golden runs and the measurement harness are unchanged.

`mischief` `[{kind, weight}]` and `mischief_rules` `{rest_every 2, reward_every 3, other_every 5, side 1.3, radius 0.75, score 25,
streak_secs 2.5, streak_bonus 10, streak_cap 6}` stand small props (`vase`, `bottle`, `can`, `yarn`) beside the lane in free slots
(every Nth free slot of a Rest, Reward or other section, a hash picks the kind and the side). Running through one knocks it over (it
flies aside and lies down, a label such as CRASH pops, and it pays `score`); a jump clears it, and a knock never hurts. Knocks within
`streak_secs` of each other build a streak that adds `streak_bonus` each, up to `streak_cap`. The classic course has none.

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
