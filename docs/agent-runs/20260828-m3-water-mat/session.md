# Session

- Branch: `master` at `d13d06d` (thin IBL SH + ACES). No clone. Worked on Emma Windows PC `D:\program\kagra`.
- Official path stays kagra-shared wgpu 30 `play_world`. Did not add Bevy, RendererV2, a second renderer, SSR, physics water, or caustics.
- Port: `Material::Water = 6` on the same shader family as Metal/Toon. `shader3d.wgsl` two scrolling fbm normals + Fresnel + `env_irradiance(reflect)`. Cheap view-angle alpha fade (no scene-depth prepass). `Globals.env.z` = elapsed seconds (frame accum, not Instant / wasm-safe).
- WorldDoc: prop name/model `water` → plane + Water. Existing `water_y` plane (Crest) now compiles as Water — dump JSON unchanged. Collectathon live water plane same family.
- Fixture: `kagra-shared/tests/fixtures/water_plane_world.json` (shore + lake + rock). `world_play.rs` untouched.
- Vertex3 location 0/1/8 and instance 2..7 unchanged. No storage buffers. No Rapier / SSAO / GI / Python API / vrm_open_world.py rewrite.
