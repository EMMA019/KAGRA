# Result

- `RhythmSession` on `kagra_shared` / `kagra.RhythmSession`
- `examples/rhythm_minimal.py` — D F J K, four tap columns, perfect/good/miss, combo, result. `--headless` auto-plays the chart.
- `examples/verify_scenarios/rhythm_smoke.json`
- `kagra-shared/tests/fixtures/rhythm_world.json`
- Backing track: `assets/cute_song_trial.wav` when present. Chart clock is 120 BPM in Rust.

Headless (lavapipe), auto-play: `phase=complete rank=S score=8400 perfect=28 good=0 miss=0 combo=28`.

`cargo test -p kagra-shared --lib --locked`: 424 passed.
`cargo clippy -p kagra-shared --all-targets --locked -- -D warnings`: clean.
`cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings`: clean.
`pytest tests -m "not golden"`: passed.
`python -m kagra.verify` on `examples/verify_scenarios/*.json`: rhythm_smoke and the other genre smokes passed on lavapipe. Skipped the three that already fail on master: `demo_live_smoke`, `physics_stack_smoke`, `rpg_town_smoke`.

Screenshots:

- `/opt/cursor/artifacts/screenshots/rhythm_play.png` — mid-chart, notes on four lanes, PERFECT
- `/opt/cursor/artifacts/screenshots/rhythm_result.png` — result S
- `/opt/cursor/artifacts/screenshots/rhythm_stage.png` — title highway from the smoke fixture
