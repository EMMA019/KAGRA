# Prompt — Mixamo walk on shared wgpu 30 (M3)

Port a thin V2 `avatar.bind_locomotion()` rest+roll retarget onto kagra-shared wgpu 30 so a VRM/glTF humanoid without a Walk clip can walk on play_world.

Not RendererV2. Do not rewrite examples/vrm_open_world.py. No full MToon, spring, look-at, morph.

Close: Walk clip kept; no clip => Mixamo walk retarget (VRM 0/1 + J_Bip_*). WASD plays; release toward bind. Crest Isle stays capsule. Tiny dump. Push origin master.
