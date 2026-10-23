# Session

API search: `scene_shadow_view_proj`, `shadow_softness`, `Globals` (448 bytes), `shader3d.wgsl` `shadow_factor`. Instancing and `cull: true` frustum AABB culling already exist, so this PR does not add a culler.

Decisions:

- Count 1 keeps the full 2048 map and the same view-projection as `scene_shadow_view_proj`. The 3×3 / 5×5 kernel is unchanged.
- Count 2 or 3 packs rings of 8 m, 24 m, and 80 m into tiles of that same depth texture. No texture array. WebGL2 stays on `texture_depth_2d`.
- Each ring is texel-snapped. Count above 1 snaps to the 1024 tile, not the full 2048.
- The three matrices go in one globals upload (`light_view_proj`, `cascade1`, `cascade2`). Cascade count is `cascade.x` and PCSS is `cascade.y`. `shadow.z` / `shadow.w` stay camera near / far for the depth prepass.
- PCSS cannot read a comparison depth on WebGL2, so each cascade pass can also write device z to an `Rgba16Float` color target. The lighting shader samples it only while `cascade.y` is on, with `textureSampleLevel` and a nearest sampler. The texture is always allocated so the bind group does not change.
- The shadow depth bias matches the WebGPU path: constant 8192, slope 1.5, clamp 0.008, on the culled and double-sided pipelines.
- `KAGRA_QUALITY` helpers match the post-pass branch (`heavy_post_allowed`, `quality_is_high`) so a merge can share them. Quality 0 forces one cascade and PCSS off even when the dump asks. High fills omitted fields only.

Lavapipe: a caster at z = -36 is outside the 24 m map. Count 1 leaves that floor lit. Count 3 darkens 28 pixels by about 190. PCSS darkens 44. The near floor stays put. Shots: `cascade-one.png`, `cascade-three.png`, `cascade-pcss.png`.
