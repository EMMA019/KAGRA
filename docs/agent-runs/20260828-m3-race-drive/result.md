# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 172 passed; 0 failed (7 race tests: dump, title, throttle, steer, lap/finish+retry, chase camera, other genres)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 548 passed, 10 deselected

python -m kagra.play_world kagra-shared/tests/fixtures/race_drive_world.json --width 640 --height 360 --seconds 2
# opened via rebuilt target/debug/examples/window.exe, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/race_drive_world.json
```

Space / Enter / click starts. WASD or arrows steer+throttle the kinematic capsule/box car. Chase camera follows. Overlay counts laps. Dump-visible lap/finish (`name` lap/finish + `flag` + `coins` count). Indoor lights stay slots 0..3.

Collectathon is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/crest_isle_world.json
```

## Notes

- Steer/throttle/lap live in `race.rs`. WorldPlay only dispatches (new/start/tick/hud).
- Dump source of truth: road boxes, finish, split, flag, box car. Chase camera name `chase`.
- No RendererV2, no VRM, no Rapier, no new ECS. DrivingScene / golden driving pixel test untouched.
- Did not land: Rapier vehicles, drifting physics, net, fighting, novel, tower placement UI, multi-lap AI racers.
