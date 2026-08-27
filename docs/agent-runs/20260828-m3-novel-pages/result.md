# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 186 passed; 0 failed (6 novel tests: dump, title, Space/click pages, stay flag+result, leave flag, other genres)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 552 passed (2 new python tests: novel fixture + smoke scenario)

python -m kagra.play_world kagra-shared/tests/fixtures/novel_pages_world.json --width 640 --height 360 --seconds 2
# opened via rebuilt target/debug/examples/window.exe, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/novel_pages_world.json
```

Space / Enter / click starts. Space / click advances overlay pages. A/D or arrows pick a 2-way choice; Space / click confirms. Stay/leave writes a dump-visible flag (`name` page/choice/stay/leave + flag enable + `coins`). Result overlay. Room camera. Indoor lights stay slots 0..3.

RPG talk is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/rpg_town_world.json
```

## Notes

- Pages / choice / flag live in `novel.rs`. WorldPlay only dispatches (new/start/tick/hud). `rpg.rs` not rewritten.
- Dump source of truth: capsule player + speaker in a room, `page` marker, flag off until choice, room camera name `room`.
- No RendererV2, no VRM, no Rapier, no new ECS. Other genre loops untouched.
- Did not land: branching save editor, portraits atlas, inventory, net, fighting combos, vehicles, typewriter text, full VN engine.
