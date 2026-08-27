# Session

1. Read docs/API_INDEX.md, world.json walker/prop dump keys, gltf_load.rs (static
   POSITION+NORMAL only), world_doc compile_scene (always MESH_CAPSULE), world_play
   step_walker, window upload-once, shader locations 2..7. No new ECS / Rapier /
   SSAO / RendererV2 / VRM.
2. CPU-skin into existing Vertex3. wgpu 30 shader unchanged (WebGL2: no storage
   buffers, no joint palette). COPY_DST vertex buf + update_world_gltf each frame.
3. Walker dump style matches props (`gltf` / `model`). `clip` is seconds into Walk
   (0 = T-pose). Capsule+head fallback when unset. world_play.rs only preserves
   those fields and advances clip while WASD; genre loops untouched aside from
   `..Default::default()` on WorldWalker literals.
4. Fixture: kagra-shared/tests/fixtures/walk_skinned.gltf (hand-authored 2-joint
   box + Walk clip, ~4KB embedded buffer) and skinned_walker_world.json.
5. Tests: cargo fmt/clippy/lib (289) / pytest 571 / wasm32 wasm,render green.
