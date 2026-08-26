# Session

- Fetched origin/master (`9ddf13d`, PR #99 World query/dump/load already merged). Branched `cursor/one-runtime-scene3d-dump-d6bc`.
- Investigated existing `Scene3D` (`kagra-shared/src/scene3d.rs`): it was a GPU-free *draw list* (camera, batches, MeshId). Mobile JSON playback is `SaveGame` / `IsleGame` (score/phase), not `world.dump()`. `kagra-shared/src/world.rs` is corridor buildings, not World3D. Did not invent a second scene type.
- Grew `Scene3D` with dump fields (props / walkers / lights / cameras / heightfield). Wire format is private `WorldDumpFile` matching `docs/schemas/world.json` version 1. `from_world_json` / `to_world_json`. GPU `batches` stay empty on ingest. Draw-list builders (`collectathon` / `driving`) use `..Default::default()`.
- `fn` / `type` are serde-renamed (`fn_name`, `kind`). Dump `fov` is degrees; the draw `Camera.fov_y` is radians, filled from `cameras[0]` so a later renderer switch has an eye.
- Fixtures: `kagra-shared/tests/fixtures/crest_isle_world.json` (open_world_height, parented coin, tile:0,0 / tile:-1,0) and `orb_rush_world.json` (flat arena, star/bomb, no heightfield). Rust roundtrips ids / positions / parent / fn / tile keys. Python `World.dump()` of matching synthetic worlds, and `World.load()` of the same fixtures. No new public game API.
- `(-12800,-12800)` fake-headless is in `kagra-core/src/window.rs` and needs the renderer switch; left it.
- Did not touch Crest Isle UV/stream, wgpu mix, VRM-on-Wasm, Rapier, SSAO, editor, goldens, M3 TRS.
