# Prompt - thin spring bones on shared wgpu 30 (M3)

origin/master is 3cfd83f (thin MToon). Next visible leftover: spring bones (hair/sleeves) on official play_world, not RendererV2.

Do not rewrite examples/vrm_open_world.py. No look-at, morph this slice. Do not copy the whole V2 spring solver — a thin chain (gravity + stiffness + one-step Verlet) is enough.

Close: parse VRM 0 secondaryAnimation / VRM 1 VRMC_springBone enough to step hair chains while the walker moves/idles. CPU skin already exists; apply spring joint rotations before skinning. Tiny 2-bone hair chain on walk_skinned.vrm / tpose_humanoid. Dump-visible hair yaw. Crest stays capsule. Push origin master.
