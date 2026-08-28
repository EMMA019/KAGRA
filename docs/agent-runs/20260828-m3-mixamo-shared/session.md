# Session

Worked on Emma's Windows PC (D:\program\kagra). No clone.

- origin/master was d35f3bd (textured CPU-skinned glTF/VRM).
- Ported V2 rest+roll (`N = W_src * delta * inv(W_src); delta_dst = inv(W_dst) * N * W_dst`) into kagra-shared `mixamo.rs`.
- Bundled a small Mixamo walk JSON (from local assets/mixamo/walk.fbx, subsampled). Not the FBX, not Emma.vrm.
- Clip-less humanoid: `tpose_humanoid.vrm` alias of the existing walk_skinned.vrm mesh with clip stripped, then Mixamo bound.
- WASD already advances `clip`; idle clip 0 now samples bind rest (Mixamo t=0 is mid-stride).
- world_play.rs unchanged. Genre loops untouched. No Rapier/SSAO/second renderer.
