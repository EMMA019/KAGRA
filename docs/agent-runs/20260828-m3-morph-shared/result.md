# Result

## Commands

```text
cargo fmt -p kagra-shared
# ok

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok
cargo clippy -p kagra-shared --all-targets --features render --offline --locked -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 326 passed; 0 failed (VRM 0/1 blink+aa parse, POSITION deltas,
# dump morph 1 moves CPU-skinned verts, idle blink + hold J,
# Crest stays capsule)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 571 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/vrm_walker_world.json
python -m kagra.play_world kagra-shared/tests/fixtures/mixamo_walker_world.json
```

Space / Enter / click starts. WASD walks. Idle blink pulses dump `morph` (blink, else aa) about every 3s; hold J (existing attack) or RPG talk forces weight 1. CPU-skinned Vertex3 follows. `--seconds` injects W. Esc quits.

Crest Isle (`python -m kagra.play_world`) stays capsule: that dump has no walker `gltf`. Same window / wgpu 30.

Dump: walker `morph` (0..1) is dump-visible. The named shape lives in the VRM (`blendShapeMaster` / `VRMC_vrm` expressions), not a new dump key besides `morph`. Point a dump `gltf` at `assets/Emma.vrm` to sample authored VRoid blink/aa on the first primitive.

## Notes

- CPU-skin still writes Vertex3. No storage buffers, no base_instance, no Rapier, no SSAO, no second renderer, no new ECS.
- Thin morph only (POSITION deltas + one named expression). No look-at.
- Remaining: look-at.
