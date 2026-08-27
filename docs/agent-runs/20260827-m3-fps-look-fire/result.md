# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 155 passed; 0 failed (7 fps tests: dump, title, fire/hurt, kill, look miss/hit, finish/retry)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 544 passed, 10 deselected

target/debug/examples/window.exe kagra-shared/tests/fixtures/fps_range_world.json --width 640 --height 360 --seconds 2
# opened, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/fps_range_world.json
```

Space / Enter / click starts. WASD walk. Mouse / arrows look. Click or J/Z/F fire (hitscan). Muzzle flash + hit overlay; dump `player.name` is `hurt` on hit; killed `target` is `enabled: false`.

Collectathon is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/crest_isle_world.json
```

## Notes

- FPS loop lives in `kagra-shared/src/fps.rs`. WorldPlay only dispatches.
- Dump source of truth: target `enabled`, player `name` fire/hurt/player.
- No RendererV2, no VRM, no Rapier, no new ECS.
- Did not land: recoil, weapons inventory, net, fighting combos, TD, novel, sprite-as-player.
