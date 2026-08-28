# Prompt - thin MToon on shared wgpu 30 (M3)

Port a thin VRM 0 materialProperties / VRM 1 MToon extras read (shadeColor + shadingToony) onto kagra-shared wgpu 30 so VRoid/VRM is not just albedo+Lambert.

Not RendererV2. Do not rewrite examples/vrm_open_world.py. No spring bones, look-at, morph, Mixamo changes.

Close: toon shade step for walker meshes that have MToon; keep GGX metal and grass/solid. Fixtures read as toon. Crest stays capsule. Push origin master.
