# Session

- Fetched origin/master (`56cf64d`, PR #98 hillside tile + PR #99). Merged into this branch. Did not touch tile UV/streaming.
- First pass stuffed dump JSON into `Scene3D`. Emma: `Scene3D` is a one-frame draw list (camera, batches, fog); collectathon/driving already build it. Dump in that struct would break mobile.
- Correct shape: persistent `WorldDoc` (`kagra-shared/src/world_doc.rs`) matching `docs/schemas/world.json`. `from_json` / `to_json` roundtrip. `compile_scene` → `Scene3D` (box / sphere / capsule / plane primitives). `Scene3D` API restored to the draw-list struct (no dump fields, no `..Default::default()` required).
- `kagra-shared/src/world.rs` stays corridor buildings. Name is `WorldDoc`, not a second `Scene3D`.
- `(-12800,-12800)` fake-headless left (needs renderer switch). No wgpu mix, no RendererV2 delete.
- Did not start the renderer switch.

