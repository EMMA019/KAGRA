# Prompt

Close THIS slice: IBL (diffuse irradiance, even a tiny SH or 1 cubemap) + tone mapping (ACES or Reinhard) on the shared wgpu 30 shader. Outdoor dumps (Crest default play_world, emma_walker_world.json) should look less flat than key+fill only. Metal GGX coins and Toon VRM must keep working.

How:
- Look at existing kagra-core V2 set_hdri / set_tonemap / set_exposure for the idea, then port a THIN version into kagra-shared render shader.wgsl + Globals. Do not rewrite vrm_open_world.py.
- Do not add Bevy as a crate. Do not copy Bevy source trees. A procedural sky irradiance or a tiny embedded studio cubemap is enough (no 4K HDR files).
- WebGL2: no storage buffers, no base_instance. Vertex3 layout (pos+normal+uv) stays compatible. Instance locations 2..7 stay.
- Dump: optional field only if it matches existing WorldDoc style; default ON for outdoor empty-light dumps is OK if tests still pass. Crest collectathon must still play.
- Relic Run UV defaults stay. No Rapier, SSAO, GI, Unreal-named extra passes.
