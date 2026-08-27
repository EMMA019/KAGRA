# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 202 passed; 0 failed (6 puzzle tests: dump, title, push, crate-on-pad solved, WASD solve, other genres)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 556 passed (2 new python tests: puzzle fixture + smoke scenario)

python -m kagra.play_world kagra-shared/tests/fixtures/puzzle_pad_world.json --width 640 --height 360 --seconds 2
# opened via rebuilt target/debug/examples/window.exe, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/puzzle_pad_world.json
```

Space / Enter / click starts. WASD (arrows also walk) into the copper crate; it slides (kinematic overlap, no Rapier). Onto the teal pad = solved. Dump-visible `name` player/pushing/solved + flag enable + `coins`. Result overlay. Room camera. Indoor lights stay slots 0..3. Pad and crate readable (inherited contact blob + metal GGX). `--seconds` walks W so the push is visible without a human.

Stealth hide is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/stealth_hide_world.json
```

## Notes

- Push / pad / flag live in `puzzle.rs`. WorldPlay only dispatches (new/start/tick/hud). Other genre loops not rewritten.
- Dump source of truth: capsule player, crate box, pad box, flag off until solved, room camera name `room`.
- No RendererV2, no VRM, no Rapier, no new ECS. Picture slice (blob/lights) inherited, not reverted.
- Did not land: sokoban editor, physics joints, net, inventory, vehicles, multi-crate levels.