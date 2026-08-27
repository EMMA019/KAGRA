# Prompt

origin/master is 175e3d6 (puzzle joints just closed, no Rapier). After puzzle,
3D humans loadable on official play_world. Priority: skinned glTF (Tripo/Blender)
walking on kagra-shared wgpu 30. NOT RendererV2. NOT VRM-as-the-vehicle.
Capsule remains fallback. VRM port is a later stretch only if this slice is
already green and pushed.

Extend gltf_load (nodes, skins, JOINTS_0, WEIGHTS_0, IBM, Walk clip). CPU-skin
into Vertex3. Walker dump `gltf` / `model`. WASD plays walk clip; idle T-pose.
Tiny fixture. Dump-visible. Tests + push origin master.
