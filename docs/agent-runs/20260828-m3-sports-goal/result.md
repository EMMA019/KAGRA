# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 208 passed; 0 failed (6 sports tests: dump, title, kick, ball-in-goal scored, WASD score, other genres)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 558 passed (2 new python tests: sports fixture + smoke scenario)

python -m kagra.play_world kagra-shared/tests/fixtures/sports_goal_world.json --width 640 --height 360 --seconds 2
# opened via rebuilt target/debug/examples/window.exe, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/sports_goal_world.json
```

Space / Enter / click starts. WASD (arrows also walk) into the metal sphere; overlap applies a kinematic impulse (no Rapier). Ball entering the goal volume = scored. Dump-visible `name` player/kicking/scored + flag enable + `coins`. Result overlay. Chase camera follows play. Outdoor lights stay slots 0..3. Pitch and goal readable (inherited contact blob + metal GGX). `--seconds` walks W so the kick is visible without a human.

Puzzle pad is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/puzzle_pad_world.json
```

## Notes

- Kick / roll / goal / flag live in `sports.rs`. WorldPlay only dispatches (new/start/tick/hud). Other genre loops not rewritten.
- Dump source of truth: capsule player, sphere ball, goal box, pitch boxes, flag off until scored, chase camera name `chase`.
- No RendererV2, no VRM, no Rapier, no new ECS. Picture slice (blob/lights) inherited, not reverted.
- Did not land: FIFA, net, inventory, vehicles, sokoban editor, multi-ball, keepers.
