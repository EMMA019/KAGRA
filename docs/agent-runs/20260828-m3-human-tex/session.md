# Session

PC: DESKTOP-463PI6Q (D:\\program\\kagra). Did not clone.

Landed on kagra-shared wgpu 30:
- Vertex3 pos+normal+uv (32 bytes). Existing primitives UV 0.
- Shader location 8 UV (instance 2..7 unchanged). Group 1 albedo texture + sampler. Default 1x1 white.
- glTF TEXCOORD_0 + baseColor PNG (data URI / bufferView). VRM0 `_MainTex` fallback.
- Fixtures: walk_albedo.png (8x8), textured walk_skinned.gltf / walk_skinned.vrm.
- Textured walker instance color white so the PNG is not teal-tinted.

Not touched: RendererV2, vrm_open_world.py, genre loops, Crest dump, Mixamo, MToon/spring/look-at/morph.
