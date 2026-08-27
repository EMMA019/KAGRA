# Close M3 human base-color texture (shared wgpu 30)

origin/master 2dc228d (thin VRM on wgpu 30). Vertex3 was pos+normal only; skinned glTF / .vrm walkers were a flat Vertex3 color.

This slice:
- Sample glTF `pbrMetallicRoughness.baseColorTexture` (or VRM0 `_MainTex` if that is what the file has).
- Add UV to Vertex3 in the smallest WebGL2-safe way (no storage buffers, no base_instance). Capsules/props/heightfield keep UV 0 / 1x1 white.
- Tiny PNG fixture; extend walk_skinned.gltf / walk_skinned.vrm. Dump still queryable. Crest Isle stays capsule.
- NOT RendererV2. Do not rewrite examples/vrm_open_world.py. No full MToon, spring, look-at, blendshapes, Mixamo.
- world_play.rs tiny. No Rapier, SSAO, second renderer. No invented APIs.

Junk never: assets/library/, assets/models/, cache/, examples/*.mp4, python/
