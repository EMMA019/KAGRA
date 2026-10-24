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

## Feel presets (`spec.feel`)

`runner_measure WORLD.json 20 --feel classic|snappy_runner|arcade_punchy|chill` overrides the preset.
Swipe/key lane change, before → after (classic → snappy_runner): 17 frames / 0.283 s → 8 frames / 0.133 s
(eased cubic-out hop, 0.17 s nominal per lane, arrival within 2 cm). Swipe-scheme long stroke 1.117 → 1.0 lanes,
post-jitter drift 0.014 → 0.0 lanes (snap on arrival). Jump, drag, bot and perf numbers are unchanged.
Spec example: `{"feel":{"preset":"snappy_runner","movement":{"jump":"forgiving"},"camera":{"look_ahead":true},"feedback":{"hit_stop":"light"}}}`.
