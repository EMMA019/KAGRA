# Result

## Commands

```text
cargo test -p kagra-shared --locked --offline --lib
# 157 passed; 0 failed (9 fps tests: dump, title, fire/hurt, kill, look miss/hit, finish/retry, eye camera tracks body, fire from eye)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok; new_for_window stays cfg(not(wasm32))

pytest tests -m "not golden"
# 544 passed, 10 deselected

python -m kagra.play_world kagra-shared/tests/fixtures/fps_range_world.json --width 640 --height 360 --seconds 2
# opened via target/debug/examples/window.exe, exited 0
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/fps_range_world.json
```

Space / Enter / click starts. WASD walk. Mouse / arrows look (first-person at capsule eye). Click or J/Z/F fire (hitscan from eye forward). Local player mesh hidden so the near plane does not clip a white interior; walker stays in the dump.

Collectathon is unchanged:

```text
python -m kagra.play_world kagra-shared/tests/fixtures/crest_isle_world.json
```

## Notes

- Eye camera lives in `fps::place_eye_camera`. WorldPlay only dispatches (new/start/tick).
- Dump source of truth: `cameras[0].name == "eye"`, walker still present. Compile skips local mesh.
- No RendererV2, no VRM, no Rapier, no new ECS.
- Did not land: recoil, weapons inventory, net, fighting combos, TD, novel, vehicles, arms mesh.
