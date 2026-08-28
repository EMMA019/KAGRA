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
# 339 passed; 0 failed (new: water_prop_and_water_y_compile_as_water_material; metal/toon/emma/crest still green)

cargo test -p kagra-shared --locked --offline --features render --test offscreen_render
# 10 passed including world_doc_offscreen_crest_isle_is_not_flat, orb_rush batches, lighting_shades_faces_differently

cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked --offline
# ok

pytest tests -m "not golden"
# 573 passed, 10 deselected
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/water_plane_world.json
python -m kagra.play_world
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json
```

Lake fixture: named `water` plane on the shared Water material (scrolling waves + Fresnel + SH reflection, no SSR). Default Crest `water_y` plane is the same material (JSON unchanged). Emma VRoid toon + gold GGX coins still the same materials.

## Notes

- Shared `shader3d.wgsl` + `Material::Water = 6` only. No cubemap, no storage buffer, no base_instance. No Bevy crate / source. No RendererV2 / Water Renderer product.
- Dump: existing `name`/`model` `water` or `water_y`. No second schema. `world_play.rs` untouched.
- Did not land: LOD/instancing, scene-depth shore fade, physics water, caustics, SSR, SSAO, GI, Rapier, 4K HDR, a second renderer, Python API, vrm_open_world.py rewrite.
