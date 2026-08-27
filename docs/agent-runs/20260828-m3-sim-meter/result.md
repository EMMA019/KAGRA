# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 215 passed; 0 failed (7 sim tests: dump, title, stand-in-zone coins, leave holds, fill flag, WASD fill, other genres)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 560 passed (2 new python tests: sim fixture + smoke scenario)

python -m kagra.play_world kagra-shared/tests/fixtures/sim_meter_world.json --width 640 --height 360 --seconds 2
# opened via rebuilt target/debug/examples/window.exe, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/sim_meter_world.json
```

Space / Enter / click starts. WASD (arrows also walk) into the metal zone pad; standing in the volume ticks `coins` (the meter). Full meter = dump-visible `name` full + flag enable + `coins==8`. Result overlay. Chase camera follows play. Indoor lights stay slots 0..3. Zone readable (inherited contact blob + metal GGX). `--seconds` walks W so the fill is visible without a human.

Sports goal is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/sports_goal_world.json
```

## Notes

- Path: **sim fallback**, not compressed agent APIs. `docs/API_INDEX.md` / SKILL / `kagra.contracts.resolve_asset` have no `enemy.chase(player)`, `avatar.state("combat")`, or `world.spawn(prefab)` on WorldDoc/play_world. Existing names only: `Prefab` class, `spawn_from`/`spawn_rule` (shelf `kagra.scriptable`), `Walk.zoom_chase` (camera). Did not mint a parallel API.
- Meter / flag / filling live in `sim.rs`. WorldPlay only dispatches (new/start/tick/hud). Other genre loops not rewritten.
- Dump source of truth: capsule player, zone box, floor box, flag off until full, chase camera name `chase`.
- No RendererV2, no VRM, no Rapier, no new ECS. Picture slice (blob/lights) inherited, not reverted.
- Did not land: enemy.chase, avatar.state, world.spawn, Unity/Godot parity, inventory, decay-on-leave, multi-zone.
