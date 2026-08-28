# Session

- Started from 9e11f05 on Emma PC `D:\\program\\kagra`. Did not clone.
- Parse lives in `kagra-shared/src/morph.rs` (VRM 0 blendShapeMaster + VRM 1 VRMC_vrm preset/custom). glTF POSITION deltas load in `gltf_load` and apply before CPU skin (same Vertex3 path as hair).
- Fixture `walk_skinned.vrm` (also Mixamo `tpose_humanoid.vrm` bytes) had no morphs. Added one POSITION target (top-front verts drop) plus VRM 0 blink/a and VRM 1 blink/aa binds. glTF walker fixture unchanged. File still tiny (~4.5 KB).
- Dump `morph` weight on WorldWalker. WorldPlay idle-blinks (0.12s close every 3s). Hold J (existing attack) or RPG talking forces weight 1. `compile_meshes` applies morph before CPU skin.
- Dump has player + walkers with the same id; tests set morph on both (same as hair).
- Crest Isle dump has no walker gltf ? still capsule, morph stays 0. Genre loops untouched (rpg.rs not rewritten; talking is a 3-line hook in world_play). No Rapier / SSAO / second renderer / look-at.
