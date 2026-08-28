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
# 318 passed; 0 failed (VRM 0/1 parse, Verlet stiffness*dt^2, fixture 2-bone hair,
# dump hair yaw moves CPU-skinned verts, idle/walk change hair, Crest stays capsule)

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

Space / Enter / click starts. WASD walks. Idle and walk both step the 2-bone hair chain; dump `hair` yaw changes over time and the CPU-skinned mesh follows. Release WASD: clip 0 / Mixamo bind, springs keep ticking. `--seconds` injects W. Esc quits.

Crest Isle (`python -m kagra.play_world`) stays capsule: that dump has no walker `gltf`. Same window / wgpu 30.

Dump: walker `hair` (radians) is dump-visible. Springs live in the VRM (`secondaryAnimation` / `VRMC_springBone`), not a new dump key besides `hair`. Point a dump `gltf` at `assets/Emma.vrm` to sample authored VRoid hair chains on the first primitive.

## Notes

- CPU-skin still writes Vertex3. No storage buffers, no base_instance, no Rapier, no SSAO, no second renderer, no new ECS.
- Thin Verlet only (gravity + stiffness + length). No colliders, sleeves, or look-at/morph.
- Remaining: look-at, blendshapes / morph.
