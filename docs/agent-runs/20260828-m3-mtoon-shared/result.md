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
# 310 passed; 0 failed (VRM 0/1 shadeColor+toony, Material::Toon on
# vrm_walker / mixamo_walker, Crest stays capsule not toon, WASD clip unchanged)

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

Space / Enter / click starts. WASD walks. Walker meshes with MToon use the shared toon shade step (shadeColor * albedo in shadow, lit albedo in light) instead of a plastic Lambert blob. Release WASD: clip 0 / Mixamo bind. `--seconds` injects W. Esc quits.

Crest Isle (`python -m kagra.play_world`) stays capsule: that dump has no walker `gltf`. Coins stay GGX metal. Grass stays grass. Same window / wgpu 30.

Dump: walker `gltf` still `walk_skinned.vrm` / `tpose_humanoid.vrm`. Shade lives in the file (VRM 0 `_ShadeColor` / `_ShadeToony` or VRM 1 `shadeColorFactor` / `shadingToonyFactor`), not a new dump key. Point a dump `gltf` at `assets/Emma.vrm` to sample authored VRoid MToon on the first primitive.

## Notes

- CPU-skin still writes Vertex3. Instance locations 2..7 unchanged; UV is location 8; MToon is location 9.
- No storage buffers, no base_instance, no Rapier, no SSAO, no second renderer, no new ECS.
- Remaining: spring bones, look-at, blendshapes / morph. Hair-only rimLift stayed leftover V2 (one global rim on the toon path).
