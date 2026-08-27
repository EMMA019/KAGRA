# Prompt

Port VRM onto the same kagra-shared wgpu 30 path as the skinned glTF walker.
Not RendererV2. Do not rewrite examples/vrm_open_world.py.

Thin slice: load .vrm (glTF-binary) through kagra-shared (nodes/skins/JOINTS/WEIGHTS,
humanoid if present). Walker dump names .vrm like .gltf. compile_scene draws VRM
mesh at walker pose. Walk clip on WASD if present, else T-pose. MToon as unlit/solid.
Tiny fixture. Crest stays capsule. No Rapier / SSAO / second renderer / new ECS.
