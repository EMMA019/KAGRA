# Runner measurement harness

```
cargo run --release -p kagra-shared --example runner_measure -- WORLD.json [RUNS] [--no-perf]
```

Prints one JSON report for a runner world (`kagra.runner --bake` output).
Everything except `perf` is deterministic, so two reports diff cleanly
before and after a tuning or preset change. Sim frames are 1/60 s.

- `feel.jump`: press-to-lift latency (frames), apex, time to apex, airtime, apex-to-land.
- `feel.lane_switch`: held stick, frames until the cat has moved one `lanes.width`.
- `feel.swipe`: the same through the engine swipe path (`gesture.rs`), plus
  whether an up-swipe 5 frames before touchdown still jumps (0.15 s buffer).
- `logic`: grace holds, an obstacle kills with the right `HIT` kind, death
  reaches the result screen, retry resets score/rings/position.
- `bot`: N seeded `script_steer` runs with reaction delay 0–4 frames, stick
  noise and a few missed jumps. Clear rate, survival, death causes.
- `course`: obstacles that can be neither jumped nor passed, back-to-back
  walls with no time to change side (`unavoidable`/`tight`), per-window
  difficulty and `spikes` (a window 1.5× harder than the one before).
  `course.combos` lists every back-to-back pair of intended actions (jump, roll, ride)
  with the smallest slack in seconds. Time comes from the real jump arc (takeoff to land, no
  fast fall: 2·v/g) and the real roll, and distance to time from the speed ramp. A pair that
  leaves under 0 s and cannot be walked round is `combo_infeasible` (error); under
  `combo_margin_secs` (0.25 s) is `combo_tight` (warn). Pairs marked `walkable_first` can be
  walked round, so they only warn.
- `reachability`: each obstacle kind is run against stand, walk, jump and roll probes and must
  pass with exactly the action its `behavior` names.
- `view`: for portrait 412×915 and landscape 16:9, at start FOV and at `fov_fast`: where the cat
  sits (fraction up from the bottom), whether the three lane edges are in frame, how far ahead a
  1 m obstacle is still 4 % of the frame tall (`readable_m`) and the time that gives at that speed
  (`reaction_s`). Warns `cat_too_high` (>0.38 in portrait), `lanes_cropped`, `short_reaction`
  (< `min_reaction_secs`, 0.8).
- `camera_clip`: for one slot of every kind, the closest the chase eye (before the engine's own
  obstacle guard) gets to any part box while the intended action is played from three lane
  starts. Under `CAMERA_CLEAR` (0.3 m) is `camera_inside` (error).
- `roll_window`: for every `roll_under` and `cat_gap` kind, at the speed cap: how long the roll
  flick can come early or late and still pass (`window_s`, `early_m`, `late_m`) and how far off
  the anchor the cat can be (`x_left_m`, `x_right_m`; a neighbour's box leaking into the opening
  shows here). Under 0.25 s is `roll_window_tight`; a cat gap with a half-width under 0.35 m is
  `gap_narrow` (both errors). Both sections feed `course.placement.issues` and `--check`.
- `perf`: tick + dump rebuild wall clock p50/p95, enabled props, and an
  estimate of instanced draw batches (one per distinct mesh). No GPU time.

`neon_cat_baseline.json` is the Neon Cat reference (20 runs, the default sectioned course with seven obstacle kinds). `neon_cat_classic.json` is the old uniform 3-crate stream (`GameSpec::legacy()`).

## Feel presets (`spec.feel`)

`runner_measure WORLD.json 20 --feel classic|snappy_runner|arcade_punchy|chill` overrides the preset.
Swipe/key lane change, before → after (classic → snappy_runner): 17 frames / 0.283 s → 8 frames / 0.133 s
(eased cubic-out hop, 0.17 s nominal per lane, arrival within 2 cm). Swipe-scheme long stroke 1.117 → 1.0 lanes,
post-jitter drift 0.014 → 0.0 lanes (snap on arrival). Jump, drag, bot and perf numbers are unchanged.
Spec example: `{"feel":{"preset":"snappy_runner","movement":{"jump":"forgiving"},"camera":{"look_ahead":true},"feedback":{"hit_stop":"light"}}}`.

## Sections and the placement validator

`runner.sections` is an optional pacing script over distance (the list repeats):
`{"kind":"intro|learn|challenge|rest|reward","length":<m>,"level":0..5,"density":0..1?}`.
`density` is the share of obstacle slots kept (deterministic per-index hash); the default is
intro 0.35, learn 0.7, challenge 1.0, rest/reward 0. Rings keep running through rests. No sections = the old uniform stream.

`course.placement` in the report (and `runner_measure WORLD.json 1 --no-perf --check`, exit 1 on any error) lists issues:

| code | severity | meaning |
|---|---|---|
| `unavoidable` | error | an obstacle blocks every lane and is too tall to jump, or back-to-back walls leave no time to change side |
| `missing_rest` | error | no empty stretch of ≥3 s (`rest_gap_secs`) in 45 s (`max_no_rest_secs`) |
| `missing_rest_section` | error | sections are set but none is rest/reward |
| `rest_not_calm` | error | a rest/reward section has obstacles (density > 0) |
| `tight` | warn | side change fits but with under half a lane switch to spare |
| `difficulty_spike` | warn | measured difficulty window ≥1.5× the previous one (rises out of a calm window are not spikes) |
| `level_jump` | warn | declared level rises by more than 1 (3 after a rest/reward) between neighbouring sections |

Finding on the old uniform stream (`neon_cat_classic.json`): `missing_rest` (longest empty gap 2.4 s), 155 obstacles in 120 s,
difficulty a flat 1.4 per window. The shipped default is the six-section script (intro, learn, challenge, rest, challenge, reward):
0 errors, 102 obstacles in 120 s, difficulty per 10 s window `[0.3, 0.6, 0.86, 1.44, 0.42, 1.76, 0.52, 0.5, 1.34, 1.68, 0.4, 1.58]`,
bot clear 0.4, survival 64.8 s. The golden runs use the classic stream.

Findings of the new checks on the default course:
- Camera: the cat sits 0.21 up from the bottom of a portrait frame at start speed and 0.25 at the speed cap (FOV 58), so it is already
  in the lower quarter. All three lane edges are in frame 4 m ahead. A 1 m obstacle reads from 20.8 m at start (2.0 s) and 16.5 m at
  the cap (0.83 s). `chase_back`, `chase_lift` and the FOV are left alone: pulling back would shrink the cat and the obstacles for
  no gain in the lower-quarter or lane-visibility numbers.
- Combos: jump → jump at the speed cap leaves 0.05 s of slack and jump → roll 0.08 s (both 14 m apart, first obstacle walkable), so they
  warn `combo_tight`. Roll → jump has 0.57 s, roll → ride 0.45 s, ride → roll 0.30 s. No pair is infeasible.
