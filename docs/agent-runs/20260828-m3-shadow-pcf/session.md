# Session

Worked on Emma Windows PC `D:\\program\\kagra` at origin/master `c2b31f1` (water + IBL/ACES). Did not clone.

Quality-only borrow from kagra-core V2: one directional ortho (DirectX Z 0..1 via glam 0.33), texel snap, 3x3 `textureSampleCompare` PCF. No Bevy, no RendererV2, no cascade, no second renderer.

Changes:
- `shader3d.wgsl`: `Globals.light_view_proj`, `vs_shadow`, group 2 depth-compare sampler, `shadow_factor` 3x3 PCF. Sun term only (toon/metal/water/Lambert). IBL/local lights unshadowed. Locations 0..9 unchanged.
- `render/mod.rs`: 2048 depth map, shadow pass then main, sky/water skip casters, contact blob stays. WebGL2: no storage buffer, no base_instance.
- `scene3d.rs`: `directional_shadow_view_proj` / snap / `scene_shadow_view_proj`. CPU tests.
- `world_play.rs` untouched. No Python API. No vrm_open_world.py.

Tests green. Next leftover: LOD/instancing.
