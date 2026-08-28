# Result

## Commands

```text
cargo fmt -p kagra-shared
# clean

cargo clippy -p kagra-shared --all-targets --offline --locked -- -D warnings
# ok (Rust 1.98 -D warnings; is_multiple_of / too_many_arguments)

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

cargo test -p kagra-shared --locked --offline --lib
# 346 passed; 0 failed (new: builder_instances_n_trees_as_one_batch; builder_splits_batches_by_material; n_trees_share_one_instance_batch; far_trees_use_billboard_not_thinned; crest_isle_vegetation_is_dense_and_instanced)

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

Crest: dense instanced trees/grass (full mesh near, billboard far). Lake / Emma dumps unchanged (no heightfield grove). Shadows / water / IBL/ACES stay.

## Notes

- Shared SceneBuilder instances same mesh+material. WebGL2: no storage buffer, no base_instance. No Bevy crate / source. No RendererV2.
- world_play.rs untouched. Dump schema unchanged. lod_radius/lod_cells already on heightfield.
- Did not land: SSAO, GI, SSR, caustics, Rapier, 4K HDR, a second renderer, Python API, vrm_open_world.py rewrite.