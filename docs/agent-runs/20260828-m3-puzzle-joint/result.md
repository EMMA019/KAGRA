# Result

## Commands

```text
cargo fmt -p kagra-shared
# ok

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 283 passed; 0 failed (8 puzzle tests: dump, title, push, crate-on-pad solved,
# WASD solve, parented lid follows crate, look-ray opens latch, other genres)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 571 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/puzzle_pad_world.json
```

Space / Enter / click starts. WASD (arrows also walk) into the copper crate; it
slides (kinematic overlap, no Rapier) and the parented lid follows (dump
`parent` = `prop:crate`). Onto the teal pad = solved. Face the cyan sensor
(right of spawn, yaw +X) and J / Z / F / click: look-ray hits the sensor AABB
and the latch name becomes `open`. Dump-visible `name` player/pushing/ray/solved
+ lid parent + latch `open` + flag enable + `coins`. Result overlay. Room
camera. Indoor lights stay slots 0..3. Pad and crate readable (inherited
contact blob + metal GGX). `--seconds` walks W so the push is visible without
a human.

## Notes

- Push / pad / fixed joint / look-ray live in `puzzle.rs`. WorldPlay only
  dispatches (new/start/tick/hud). Did not touch world_play.rs.
- Fixed joint uses existing dump `WorldProp.parent` (one-level id). Lid local
  offset is kinematic, not Rapier. Ray is an AABB query along walker yaw
  (`WalkInput.attack`); crate/walls do not occlude the sensor.
- Rapier was not needed: two bodies linked by a parent offset, and a line
  query vs a box, are already expressible in the puzzle module.
- No RendererV2, no VRM, no new ECS, no extra lights/SSAO.
- Remaining puzzle: hinge (this slice is fixed, not a rotating hinge), sokoban
  editor, n-level TRS, net, inventory, vehicles, multi-crate levels.
