# Session

- Same branch as PR #100 (`cursor/one-runtime-scene3d-dump-d6bc`). Did not open a new PR.
- Added `Renderer::upload_compile_meshes` / `Renderer::render_world_doc` and free `render_world_doc(doc, w, h)` in `kagra-shared/src/render/mod.rs`. wgpu 30 offscreen only. Uploads `compile_meshes()` so batch `MeshId`s match GPU slots, draws `WorldDoc::compile_scene`, reads RGBA8.
- Did not touch `kagra-core` `RendererV2`, `window.rs`, or `(-12800,-12800)`. No wgpu 0.19 mix. Desktop window path stays.
- GPU-free tests stay in `world_doc.rs` (roundtrip + compile_scene + compiled batch ids ⊆ compile_meshes). GPU tests in `tests/offscreen_render.rs` skip when `new_offscreen` has no adapter.
- Example: `cargo run -p kagra-shared --features render --example offscreen -- 640 360 world.png world`.
- Capsules / boxes / sphere-as-box stand-in. No extra PNG goldens.
