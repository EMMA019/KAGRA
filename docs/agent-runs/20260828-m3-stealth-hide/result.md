# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 192 passed; 0 failed (6 stealth tests: dump, title, hide volume, cone caught, hidden-then-exit clear, other genres)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 554 passed (2 new python tests: stealth fixture + smoke scenario)

python -m kagra.play_world kagra-shared/tests/fixtures/stealth_hide_world.json --width 640 --height 360 --seconds 2
# opened via rebuilt target/debug/examples/window.exe, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/stealth_hide_world.json
```

Space / Enter / click starts. WASD (arrows also walk) into the hide crate; the guard capsule faces a cone (red floor beam). Unseen reach of the green exit = clear. Walk into the cone without hiding = caught. Dump-visible `name` player/hidden/clear/caught + flag enable + `coins`. Result overlay. Room camera. Indoor lights stay slots 0..3.

Novel pages is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/novel_pages_world.json
```

## Notes

- Hide / cone / exit / flag live in `stealth.rs`. WorldPlay only dispatches (new/start/tick/hud). Other genre loops not rewritten.
- Dump source of truth: capsule player + guard, hide box, exit box, flag off until result, room camera name `room`.
- No RendererV2, no VRM, no Rapier, no new ECS. Other genre loops untouched.
- Did not land: AI patrol editor, noise meters, line-of-sight occluders beyond the hide volume, net, inventory, vehicles, full VN.