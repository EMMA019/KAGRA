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
- `perf`: tick + dump rebuild wall clock p50/p95, enabled props, and an
  estimate of instanced draw batches (one per distinct mesh). No GPU time.

`neon_cat_baseline.json` is the Neon Cat reference (20 runs).

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

Finding on the shipped Neon Cat: the uniform stream has `missing_rest` (longest empty gap 2.4 s; docs/measure/neon_cat_baseline.json).
`docs/measure/neon_cat_sectioned.json` is the same game with a six-section script (intro, learn, challenge, rest, challenge, reward):
0 errors, 155 → 102 obstacles in 120 s, difficulty per 10 s window `[0.3, 0.6, 0.8, 1.3, 0.3, 1.5, …]` instead of a flat 1.4,
bot clear 0.45 → 0.45, survival 63.5 s → 64.1 s. The default spec is unchanged (golden runs unchanged).
