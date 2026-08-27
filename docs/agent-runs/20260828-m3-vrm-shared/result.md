# Result

## Commands

```text
cargo fmt -p kagra-shared
# ok

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok (also --features render)

cargo test -p kagra-shared --locked --offline --lib
# 294 passed; 0 failed (VRM GLB + humanoid, T-pose without clip,
# walker dump .vrm queryable, compile_scene mesh not capsule,
# Crest stays capsule, WASD plays Walk clip like skinned glTF)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# ok (same suite as skinned slice; 10 golden deselected)
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/vrm_walker_world.json
```

Space / Enter / click starts (this dump is already playing). WASD walks; the
CPU-skinned VRM mesh plays the Walk clip (same Vertex3 path as `walk_skinned.gltf`).
Release WASD: clip 0, T-pose. Yaw from existing walker. Capsule is not drawn
when `gltf` names a `.vrm`. `--seconds` injects W so the walk is visible without a human.
Esc quits.

Crest Isle (`python -m kagra.play_world`) stays capsule: that dump has no walker
`gltf`. Same window / wgpu 30.

Dump: set walker `gltf` to a `.vrm` path (or `walk_skinned.vrm` for the bundled
fixture) and `model` to `capsule` (fallback name). Same keys as skinned glTF.

## Landed vs leftover

Landed on wgpu 30 / kagra-shared:
- `.vrm` as glTF-binary (GLB magic, JSON + BIN)
- nodes / skins / JOINTS_0 / WEIGHTS_0 / inverseBindMatrices
- VRM 0 `extensions.VRM.humanoid` and VRM 1 `extensions.VRMC_vrm.humanoid`
- Walk clip on WASD; bind-pose if no clip
- unlit/solid Vertex3 (walker body color)

Leftover on RendererV2 / `examples/vrm_open_world.py`:
- full MToon, base-color texture, hair rim
- spring bones, look-at, blendshapes
- Mixamo retarget
