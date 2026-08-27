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
# 297 passed; 0 failed (UV+baseColor on walk glTF/VRM, Vertex3 32-byte stride,
# walker dump still queryable, Crest stays capsule, WASD clip unchanged)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 571 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/skinned_walker_world.json
python -m kagra.play_world kagra-shared/tests/fixtures/vrm_walker_world.json
```

Space / Enter / click starts. WASD walks; the CPU-skinned mesh plays Walk and now samples the bundled 8x8 baseColor (skin/hair/shirt/pants), not a flat teal box. Release WASD: clip 0, T-pose. `--seconds` injects W. Esc quits.

Crest Isle (`python -m kagra.play_world`) stays capsule: that dump has no walker `gltf`. Same window / wgpu 30.

Dump: walker `gltf` is still `walk_skinned.gltf` / `walk_skinned.vrm` (queryable). Texture lives in the file (`baseColorTexture` / VRM0 `_MainTex`), not a new dump key.

## Export (Blender / Tripo)

Same as the skinned/VRM slices, plus:
- `TEXCOORD_0` (FLOAT VEC2, or normalized UNSIGNED_SHORT / UNSIGNED_BYTE)
- `pbrMetallicRoughness.baseColorTexture` (embedded PNG data URI or bufferView)
- VRM 0: `extensions.VRM.materialProperties[].textureProperties._MainTex` if PBR is missing

External image URIs are not this slice. MToon / hair rim is not this slice.

## Notes

- CPU-skin still writes Vertex3. Instance locations 2..7 unchanged; UV is location 8.
- No storage buffers, no base_instance, no Rapier, no SSAO, no second renderer, no new ECS.
- Remaining: MToon, spring bones, look-at, blendshapes / morph, Mixamo retarget.
