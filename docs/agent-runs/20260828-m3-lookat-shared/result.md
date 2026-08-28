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
# 332 passed; 0 failed (VRM 0/1 lookAt parse, rest/roll not raw bind*delta,
# dump look_yaw/look_pitch moves CPU-skinned verts, idle chase-cam look,
# morph blink + hair + Mixamo kept, Crest stays capsule)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 571 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/vrm_walker_world.json
```

Space / Enter / click starts. WASD walks. The head yaws/pitches toward the chase camera (dump `look_yaw` / `look_pitch`). Idle blink, spring hair, Mixamo rest+roll, MToon, albedo, CPU skin stay. `--seconds` injects W. Esc quits.

Crest Isle (`python -m kagra.play_world`) stays capsule: that dump has no walker `gltf`. Same window / wgpu 30.

Dump: walker `look_yaw` / `look_pitch` (radians, clamped) are dump-visible. LookAt maps live in the VRM (`firstPerson` / `VRMC_vrm.lookAt`), not a new dump key besides those two. Point a dump `gltf` at `assets/Emma.vrm` to sample authored VRoid head/eyes on the first primitive.

## Notes

- CPU-skin still writes Vertex3. No storage buffers, no base_instance, no Rapier, no SSAO, no second renderer, no new ECS.
- Thin look-at only (head yaw/pitch toward camera; eyes if those bones exist). Neck uses Mixamo rest+roll.
- This closes the official play_world VRM leftover (skinned VRM + Mixamo + MToon + albedo + spring + morph + look-at). 2D projectile+room is a later genre gap, not this slice. RendererV2 / `examples/vrm_open_world.py` still exist and were not rewritten.
