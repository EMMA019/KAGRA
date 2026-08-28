# Prompt

Work on Emma's Windows PC. Do NOT clone. Repo D:\program\kagra (EMMA019/KAGRA).

origin/master is now 01cabc1 (2D projectile+room just closed). Listed M3 genre gaps and the VRM port onto shared wgpu 30 are closed. Emma checks around 18:00 JST. Make a first-class official play dump so she can walk her VRoid on play_world without RendererV2.

assets/Emma.vrm is a true T-pose VRoid (J_Bip_*). Mixamo rest+roll already lives on kagra-shared. Do NOT rewrite examples/vrm_open_world.py. Do NOT surprise-swap default Crest Isle collectathon (`python -m kagra.play_world` with no args stays capsule collectathon — that close condition stays).

Close THIS slice:
- A dump under kagra-shared/tests/fixtures/ (e.g. emma_walker_world.json) whose walker `gltf` points at Emma.vrm via existing resolve_asset / repo-relative path (do not invent APIs, do not hardcode D:\).
- If Emma.vrm is not in git (large), do not commit the binary. The dump may reference `assets/Emma.vrm`; tests should skip or fall back to tiny tpose_humanoid.vrm when the file is missing so CI stays green.
- WASD Mixamo-walks Emma when the file is present (clip-less T-pose). Keep MToon/albedo/spring/morph/look-at that already apply to VRM walkers.
- Short log + README try line. world_play.rs tiny. No Rapier/SSAO/second renderer/new ECS.
