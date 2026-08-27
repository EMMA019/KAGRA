# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 165 passed; 0 failed (8 td tests: dump, title, path walk, tower hit/hurt, clear/retry, leak name+count, overview camera, other genres)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 546 passed, 10 deselected

python -m kagra.play_world kagra-shared/tests/fixtures/td_lane_world.json --width 640 --height 360 --seconds 2
# opened via rebuilt target/debug/examples/window.exe, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/td_lane_world.json
```

Space / Enter / click starts. Creeps walk the box path. The tower auto-hits in range. Overlay counts live creeps / leaks. Dump-visible leak (`name` leaked + `coins` count) or clear (`name` clear). Indoor lights stay slots 0..3.

Collectathon is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/crest_isle_world.json
```

## Notes

- Path/spawn/hit live in `td.rs`. WorldPlay only dispatches (new/start/tick/hud).
- Dump source of truth: waypoint boxes, tower, creeps. Overview camera name `overview`.
- No RendererV2, no VRM, no Rapier, no new ECS.
- Did not land: player-placed towers UI, multiple waves editor, fighting combos, novel, racing vehicles, net, lives UI.
