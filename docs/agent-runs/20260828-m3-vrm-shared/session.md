# Session

- First: origin/master CI clippy failed on 1b3ea36 (Rust 1.98 `chunks_exact_to_as_chunks`).
  Fixed `gltf_load.rs` to `as_chunks`, pushed 4fe9f70.
- Then: wrap `walk_skinned.gltf` as a 2.3 KiB VRM 0 GLB with hips/chest humanoid extras.
- GLB split + buffer-0 BIN in `parse_gltf`. Walker dump `gltf: walk_skinned.vrm`.
- Crest dump still has no walker glTF; capsule stays.
- examples/vrm_open_world.py untouched (RendererV2 leftover).
