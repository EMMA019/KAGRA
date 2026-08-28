# Result

## Commands

```text
cargo fmt -p kagra-shared
# clean

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 341 passed; 0 failed (new: directional_shadow_maps_focus_and_caster; shadow_center_snap_is_stable_for_sub_texel_eye_move)

cargo test -p kagra-shared --locked --offline --features render --test offscreen_render
# 10 passed including world_doc_offscreen_crest_isle_is_not_flat, lighting_shades_faces_differently

cargo test -p kagra-shared --locked --offline --lib --features render shader3d_has
# ok (PCF / vs_shadow / no storage / locations)

cargo test -p kagra-shared --locked --offline --lib --features render globals_env
# ok (Globals 400)

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 573 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world
python -m kagra.play_world kagra-shared/tests/fixtures/water_plane_world.json
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json
```

Crest / lake / Emma VRoid: directional sun umbra + 3x3 PCF on ground and capsule/human, contact blob kept. Water and IBL/ACES unchanged.

## Notes

- Shared `shader3d.wgsl` + one 2048 depth map. Depth compare PCF (WebGL2 sampler2DShadow). No cubemap, no storage buffer, no base_instance. No Bevy crate / source. No RendererV2 / cascade.
- `world_play.rs` untouched. Dump schema unchanged. Contact blob stays.
- Did not land: LOD/instancing, SSAO, GI, SSR, caustics, Rapier, 4K HDR, a second renderer, Python API, vrm_open_world.py rewrite.
