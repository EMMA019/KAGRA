# Session

- Started from 3cfd83f on Emma PC `D:\\program\\kagra`. Did not clone.
- kagra-core already has full SpringBone (colliders, sleeves, virtual tails). kagra-shared does not depend on kagra-core; copied the thin idea (parse V0/V1 + stiffness*dt^2 Verlet) into `kagra-shared/src/spring.rs` with glam.
- Fixture `walk_skinned.vrm` (also Mixamo `tpose_humanoid.vrm` bytes) had no springs. Added Hair + HairTip nodes, IBM, top-vert weights, VRM 0 `secondaryAnimation.boneGroups`. glTF walker fixture unchanged.
- Dump `hair` yaw on WorldWalker. WorldPlay steps Verlet each tick (idle and walk). `compile_meshes` applies hair before CPU skin. clip<=0 stays bind/rest so Mixamo idle is T-pose.
- First mesh test failed because dump has player + walkers with the same id; compile reads walkers first. Tests now set hair on both.
- Crest Isle dump has no walker gltf — still capsule. Genre loops untouched. No Rapier / SSAO / second renderer.
