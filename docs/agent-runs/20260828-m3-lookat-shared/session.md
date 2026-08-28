# Session

- Started from 1fb0000 on Emma PC `D:\program\kagra`. Did not clone.
- Parse lives in `kagra-shared/src/lookat.rs` (VRM 0 firstPerson lookAt maps + VRM 1 VRMC_vrm.lookAt). Head yaw/pitch apply in `gltf_load` after clip locals, before hair, with Mixamo `retarget_delta` (identity src rest, dest rest world). Not raw bind*delta. Eyes get range-map outputScale if those bones exist.
- Fixture `walk_skinned.vrm` (also Mixamo `tpose_humanoid.vrm` bytes) had no Head. Appended Head node 4 (Chest -> Head -> Hair), JSON-only so Hair/HairTip bind world (and IBM) stay put. VRM 0 firstPerson + VRM 1 lookAt + humanoid head. File still tiny (~5.4 KB). `vrm_walker_world.json` unchanged.
- Dump `look_yaw` / `look_pitch` (radians, clamped to lookAt inputMaxValue) on WorldWalker. WorldPlay `follow_camera` then `step_look` aims at the chase camera. `compile_meshes` passes dump angles into CPU skin.
- Dump has player + walkers with the same id; tests set look on both (same as hair/morph).
- Crest Isle dump has no walker gltf so still capsule, look stays 0. Genre loops untouched. No Rapier / SSAO / second renderer / RendererV2.
