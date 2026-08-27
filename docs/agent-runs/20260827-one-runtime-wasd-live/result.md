# Result

- `kagra-shared/src/world_play.rs` — WASD / look tick mutates `WorldDoc` walker + camera
- `WorldDoc::compile_scene` / `compile_meshes` — heightfield grid + glTF slots
- `kagra-shared/examples/window.rs` — live wgpu 30 window (`python -m kagra.play_world`)
- Official Crest play: `python -m kagra.play_world` (capsule). Leftover VRM: `examples/vrm_open_world.py`
- Try: `python -m kagra.play_world` — WASD, mouse/arrows look (click to recapture), Space jump
- `Renderer::new_for_window` is desktop-only (`cfg(not(target_arch = "wasm32"))`)
- Python `World.load` keeps primitive + `gltf` alias (`crate.glb` does not drop the Crest crate)

Verify (after wasm gate + Crest `crate.glb` + load alias):

```
pytest tests -m "not golden"                          # 530 passed, 1 skipped, 10 deselected
python3 tools/gen_api_index.py --check                 # OK, 409 entries
cargo test -p kagra-shared --locked                    # 121 lib (GPU-free WorldPlay / compile_scene)
cargo test -p kagra-shared --features render --locked  # 122 lib + 10 offscreen
cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings
cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --locked
```

The 1 skip is a shared GPU helper without a presentable adapter (same class as #102). Window present is Emma's desktop Vulkan/Metal.

GPU-free coverage that this mountain asked for:

- `world_play::tests::wasd_tick_moves_walker_on_heightfield` — WASD updates walker in WorldDoc
- `world_doc::tests::compile_scene_emits_heightfield_and_gltf_batches` — heightfield + cube.glb
- Crest fixture compile asserts `MESH_HEIGHTFIELD` + glTF slot (`crate.glb`)
- `world_play::tests::look_updates_camera_in_world_doc` / `strafe_and_idle_tick_orb_rush_floor` — tick moves the walker
- pytest `test_walk_input_from_keys_wasd_and_look` / `test_shared_world_fixtures_load_in_python` (crate + gltf alias)

