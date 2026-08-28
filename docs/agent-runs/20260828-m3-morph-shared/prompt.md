# Prompt - blendshapes / morph targets on shared wgpu 30 (M3)

origin/master is 9e11f05 (thin spring bones). Next leftover: blendshapes / morph targets on official kagra-shared wgpu 30. NOT RendererV2. Do not rewrite examples/vrm_open_world.py. No look-at this slice.

Close: parse glTF morph targets (POSITION deltas) and VRM 0 blendShapeMaster / VRM 1 VRMC_vrm expressions enough to apply one named shape (blink / aa) onto the CPU-skinned Vertex3. Dump-visible expression weight. Idle blink / hold J. Tiny morph on walk_skinned.vrm. Crest Isle stays capsule. Push origin master.
