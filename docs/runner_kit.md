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

`perch_route` `[ac_unit | pipe | sign, ...]` (a name, or `{kind, height}`; default heights 0.6, 1.1, 1.6 m) and `perch_rules` `{enabled false, rest_every 4,
reward_every 4, other_every 10, side 1.35, half_width 0.7, step_len 3.6, step_up 0.65, land_score 30, run_score 40, route_bonus 120,
coins_per_step 3, ramp_len 1.4}` lay a staircase of ledges (an AC unit, a pipe, a sign) end to end along one side of a free slot, in the colour of
`style.codes.rideable` with a lit top and edges. A prop makes way for a route in its slot. A straight slope of `ramp_len` metres
leads up to each rising step (0 = square risers), so a lane switch into the route needs no jump: the cat rides up the slope, or
glides up a step it walks into from the side (a first step within `step_up` of the street is stepped onto; one higher takes a jump),
and keeps running on top. A jump pressed anywhere before or on the slope still comes down on the first step (a late or early one
at running speed gets a lower hop, a small hop that meets the slope lands on it). Each next step is at most `step_up` above the
one before, so the cat walks up without another jump; both sides refuse a route a jump cannot reach, a step that is too high, more than 6 steps, or one longer than a slot. A ledge
is passed through when the feet are below its top minus `step_up`, so a perch has no collider and cannot hurt. Landing pays
`land_score`, time on a ledge `run_score` per second, reaching the last step `route_bonus` (each once per route), and each step has
`coins_per_step` coins along its top. `ramp_len` is 0..=3 and at most half a `step_len`. An empty `perch_route` (the classic course, `legacy()`) has none.

**The stair lane and the upper course are off by default** (`perch_rules.enabled false`, `deck_rules.enabled false`): on a phone the steps and decks were hard to see, so the default Neon Cat course is the plain street again. Nothing was removed: set `"perch_rules": {"enabled": true}` and/or `"deck_rules": {"enabled": true}` to bring each back with its placement, sim, side-hit rules, validators and measure checks (the stair deck needs both). The side-hit rule for a train's ramp stays on either way.

`deck_rules` `{enabled false, rest_every 3, reward_every 2, other_every 8, lanes [-1, 0, 1], length 7, height 2, half_width 0.6,
stair_entry true, stair_len 3, land_score 60, run_score 60, coins 6}` is the upper course (2階): a short raised deck directly above
a lower lane, open underneath, in every n-th free slot of its section. The lane is picked from `lanes` by the slot's hash (lane
numbers times `runner.lanes.width`). `height` must be above a plain jump (about 1.15 m + the body) and below the high jump's apex minus
0.3 m, so only a Superjump gets up; a plain jump passes under it. A Superjump orb (`items`, in the lane of the deck) lies half a slot
before it. With `stair_entry` the stair lane's last step carries on over a `stair_len` m slope up to a deck (the last step must be
within `perch_rules.step_up` of `height`), so a cat that took the stairs with no jump gets up too. Landing pays `land_score`,
standing pays `run_score` per second, `coins` lie along the top. The deck ends in a drop: the spec refuses a deck whose step off the
end, at the speed cap, would land on the next obstacle (`DECK_OBSTACLE_REACH` 2 m + `DECK_EXIT_MARGIN` 1 m), a height outside the
high jump, a lane outside the rails, or a stair deck that does not fit its slot (Rust `validate_decks`, Python `_check_decks`).
`enabled false` (the default, and `GameSpec.legacy()`) builds nothing and the checks are skipped. The measure harness plays a lone deck and a stair deck at the start speed
and at the cap (`decks` in the report, `deck_*` codes in `course.placement.issues`).

Side contact is a hit. A Ramp part (the wedge in front of a train) and the stair lane's slopes and risers kill the cat that runs
into them from the side once the surface at that point is more than `body.step_tol` (wedge) or `perch_rules.step_up` (stairs) above
its feet. The front ride-up, the toe of a wedge, a jump across, and the stair lane's first step (a lane switch-in, #192) are not hits.

`chaser` `{enabled, secs 10, near_gap 1.2, far_gap 1.7, perch_drain 2.5, scare_score 50, color}` is the cleaning robot, **off by default** (`enabled: false`; Neon Cat and Meteor Cat run without it and a hit just
gives the usual `invuln_secs` of grace). Set `"chaser": {"enabled": true}` to bring it back. When enabled, a hit that
does not end the run (so only with `lives.count` above 1) calls it. For `secs` it rolls behind the cat, a blinking beacon, a lit ring
and a pulse on the street, a "ROBOT" bar in the HUD. While it is there a second hit is the end. It falls back from `near_gap` to
`far_gap` as its time runs down and then gives up. Its time runs `perch_drain` times faster while the cat stands on a ledge or boosts
(a cleaner cannot climb). A `meow` pick-up scares it away (`scare_score`); with no robot a meow is just +1 life, or +100 score when the lives are full. Its `far_gap` is capped at 2.2 m because the chase view is
low and close; a unit test projects the robot through the real camera at both gaps and across the lanes.

Item icons are checked as pictures. The chase camera looks along +Z, so screen-right is world -X; `icon_props` mirrors the picture
into screen space, and the test `item_icons_read_the_right_way_round_from_the_chase_camera` rasterises every icon's primitives
exactly as the camera sees them (at rest and swayed to either side) and asserts, for instance, that the boost chevron's tip points
away from the cat and the `two_x` and `arrow` read the right way round (run it with `--nocapture` to see the pictures).

`style.codes` is the colour language, one neon colour per thing to do, used for the obstacle edge and its glow:
`jump_over` amber (low, hop), `roll_under` pink (high or hanging), `solid` red (go round), `cat_gap` violet (roll through),
`rideable` blue (run on it), `pass_through` mint (gates and arches, never a threat). A kind may override with `color`.
`validate` rejects two codes that are too alike. Gameplay is by action (jump, roll, lane), not by name, so a skin can retheme the
palette and labels without touching mechanics. The default course is generated, not hand-placed: `GameSpec::legacy()` is the old
three-crate stream the golden tests replay.

Models: `assets/models/kenney/space-kit/` (Kenney Space Kit, CC0), fetched by `tools/fetch_assets` with its record.

## Night look, sound and goals (Neon Cat)

Three more `game` sections, all plain JSON, all with `enabled`. Neon Cat has them **on**; `GameSpec::legacy()` (the classic course the
golden runs replay) has them **off**, as does Meteor Cat's `atmosphere` and `missions`, so no recorded run or measurement baseline moves.
Write only what you change; every other field keeps its default.

`atmosphere` is the Stray-style night street on top of the neon base look. No daytime colour and no city backdrop. The picture tier
(`quality`: `low` | `mobile` | `high`; the phone page picks `mobile` for Neon Cat, `?quality=low|mobile|high` overrides it; `?fx=low|mid|high` sets only how much night is drawn; `?adapt=0` turns off the page's render-scale step-down) decides how much
is drawn. A missing texture falls back to the plain dark wall.

| Field | Default | What it does |
|---|---|---|
| `rain` `{enabled, count_low 24, count_mobile 36, count_high 96, reach 26, behind 5, fall_speed 11, length 0.9, slant 0.12, width 1.1, color [170,205,255,125], splash}` | on | slanted streaks that fall past the camera; `splash` adds ripples on the street |
| `signs` `{enabled, cell 4, density_low 0.2, density_mobile 0.4, density_high 0.55, palette, rows [3.5, 4.9], size 1.7, line_width 2.4, glow 1.6, flicker 0.3}` | on | neon signs on the walls: a dark plate and glowing glyph strokes, placed by a seeded hash per `cell` metres, one colour from `palette` (1 to 16 RGB triples) |
| `wall` `{enabled, texture_low "", texture_mobile concrete_wall_s.png, texture_high concrete_wall.png, tint, segment 12, reach 48}` | on | concrete on both walls, tiled in `segment`-metre panels out to `reach`. An empty path means that tier keeps the plain wall (`low` does by default) |

The textures are made by `tools/gen_assets/cat_wall.py` (procedural, seeded, CC0; `--check` verifies the PNGs match the script). Point a
tier at your own PNG (under `assets/`) and list it in `bundle_assets` so the page stages it.

`audio` names the file for the music and for each effect. The page plays them with Web Audio (the sim only raises cue names), so one
set of files works on desktop, Android and iPhone browsers.

| Field | Default | What it does |
|---|---|---|
| `master` 0.8, `bgm_volume` 0.7, `sfx_volume` 1.0 | | the three volume knobs, 0 to 1 |
| `bgm {file, volume 0.55, fade_secs 1.2}` | `assets/audio/neon_cat/bgm.wav` | the loop. The run starts and stops it, with the fade |
| `sfx {cue: {file, volume, rate}}` | 13 cues | `rate` is the playback speed (0.25 to 4: it changes pitch). The table is replaced as a whole, so list every cue you want to hear; an empty `file` silences that one |

The cue names: `jump`, `roll`, `coin`, `card`, `item`, `meow`, `perch`, `knock`, `robot_on`, `robot_off`, `hit`, `gameover`, `mission`.
Files may be `.wav`, `.ogg` or `.mp3` (any asset-relative path; no `..`, no URL). Music gain is `master x bgm_volume x bgm.volume`
and an effect's is `master x sfx_volume x cue.volume`.

To use your own sound, drop the files under `assets/audio/<your-set>/` and point the cues at them:

```json
"audio": {"bgm": {"file": "assets/audio/mine/night.mp3", "volume": 0.5},
          "sfx": {"coin": {"file": "assets/audio/mine/coin.ogg"}, "hit": {"file": "assets/audio/mine/hit.mp3"}}}
```

`build_browser_preview.py` stages everything under `assets/audio/` (`.wav`, `.ogg`, `.mp3`). Add a line to `assets/LEDGER.json` for
each pack you add (`tools/asset_ledger.py check` fails on an unrecorded one) and run `tools/asset_ledger.py credits --write`.
The shipped sounds are placeholders made by `tools/gen_assets/cat_audio.py` (synthesised, CC0). On a phone, browsers keep the page silent
until the first touch: the page opens the audio on the first tap, key or click and then starts the loop. The sound button (key `M`)
mutes and is remembered; `?mute=1` starts muted and `?audio=0` turns audio off for that visit. An iPhone's silent switch is
respected by the system (the page asks for "playback" audio where Safari supports it).

`missions` `{enabled, list [{kind, target, label}], draw 3, bonus 0, hud}` are the goals for each run: `coins` (collect N), `gates`
(pass N gates), `passes` (leave N obstacles behind, hit or not) and `perch` (perch N times). `list` is the pool, in order (up to 5;
`label` overrides the text), and a run shows the first `draw` goals the course can offer (0 = all of them). A perch needs a ledge, so
it is left out of the pool unless `perch_rules.enabled` or `deck_rules.enabled` is true. The default pool is 40 coins, 5 gates,
1 perch, 30 passes, so the plain street shows coins, gates and passes, and the stair lane or the decks swap the passes for the perch.
Progress is counted from the run's own counters, shown on the title card, as a small HUD strip (`hud`) and on the result, with a
banner and the `mission` cue when one is done. `bonus` is score added per finished goal and is **0** by default, so missions never
change a score unless you ask. The high score is shown on the title and the result; the page keeps it in `localStorage` and passes it
in as the world's ``best`. The #181 ranking is unchanged.

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
- `lives`: `LivesSpec` (the `lives` JSON section) and `Lives` (hits left, the
  blink after one, `hit` -> `Absorbed` or `Fatal`, life pick-up, blink phase)
- `missions`: `MissionsSpec` (the `missions` JSON section, pool + `draw`),
  the `Counters` trait a kit implements to hand over its numbers, and
  `Tracker` (announce each goal once, write the `[{kind,label,target,value,done}]`
  the page reads)
- `result`: `ResultState` (the game-over cue that waits for the freeze, the
  banner, the BEST chip rule)

`Core` carries `stats`, `juice`, `lives`, `missions` and `result`, so a new kit
gets the 3 lives, the score, the mission strip and the game-over flow by
filling in a `LivesSpec` / `MissionsSpec` and calling `lives.hit(..)`,
`missions.update(..)` and `result.take_gameover(..)`; it only draws its own
HUD. The BEST score itself is `GameFlow::best_score` (flow.rs). Python mirror:
`kagra/run_rules.py`; both sides replay
`kagra-shared/tests/fixtures/run_rules_vectors.json`. What stays in a kit:
what a hit is (hitboxes), the sparks and sounds of it, the counters a goal
reads (`neon_missions::Counts` for Neon Cat), the paw-row / mission-strip
painting, and the pick-up effects.

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

## Adding a kit to the session

The session holds a `Box<dyn KitHost>` (`kagra-shared/src/kit_host.rs`). To plug a new kit in:

1. Write the kit on `game_core` (`Core`, `Kit`, `tick_frame`) and give it `is_<name>_dump(json)` and `from_json`.
2. `impl KitHost for YourKit`: phase name, document, dump/state JSON, confirm/title, pointer/key/walk, `tick`
   (usually a call to `tick_frame`), `hud`. `touch`, `uses_swipe` and `set_debug` have defaults.
3. Add one `(detect, load)` line to `kit_host::KITS`.

The session, the wasm shell and the playtest hand-off need no other change. Tests that need the typed kit use
`session.cat()` or add a small downcast helper next to `cat()` (`session.hop()` is HOP-BEAT's, with the audio-clock,
latency, direct-play and revive hooks the page calls).
