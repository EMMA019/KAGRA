# Result

## Commands

```text
cargo fmt -p kagra-shared
# ok

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 289 passed; 0 failed (gltf_load skin+Walk sample, walker dump gltf queryable,
# compile_scene uses glTF slot not capsule, clip moves verts, WASD clip/idle T-pose)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 571 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/skinned_walker_world.json
```

Space / Enter / click starts (this dump is already playing). WASD walks; the
CPU-skinned mesh plays the Walk clip (vertices change; dump `clip` advances).
Release WASD: clip 0, T-pose. Yaw from existing walker. Capsule is not drawn
when `gltf` is set. `--seconds` injects W so the walk is visible without a human.
Esc quits.

Crest Isle (`python -m kagra.play_world`) stays capsule: that dump has no walker
`gltf`. Same window / wgpu 30.

## Export (Blender / Tripo)

glTF 2.0 (`.gltf` or `.glb`) with:
- one mesh primitive, TRIANGLES, POSITION, optional NORMAL
- JOINTS_0 + WEIGHTS_0 (4 influences; UNSIGNED_SHORT or UNSIGNED_BYTE joints)
- skins[0].joints + inverseBindMatrices
- nodes hierarchy; first animation named `Walk` / `walk` (else clip 0)
- LINEAR rotation/translation/scale. CUBICSPLINE skipped. Morph targets no.

Blender: select armature+mesh, File > Export > glTF 2.0, format glTF Embedded
or glTF Separate, include Skinning + Animation. Do not export NLA as many
unrelated clips if you want Walk to be picked by name.

Tripo: export glTF 2.0 with skin. Mixamo retarget is later. FBX / VRM are not
this slice.

Dump: set walker `gltf` to the file path (or `walk_skinned.gltf` for the
bundled fixture) and `model` to `capsule` (fallback name). Same keys as props.

## Notes

- CPU-skin into Vertex3. Shader3d / instance locations 2..7 unchanged.
- No Rapier, no SSAO, no second renderer, no new ECS. VRM not started.
- Remaining: textures, morph targets, multi-clip blend, Mixamo, VRM port.
