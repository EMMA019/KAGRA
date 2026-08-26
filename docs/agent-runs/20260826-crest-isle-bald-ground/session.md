# Session — Crest Isle bald meadow (ハゲ)

Started from `origin/master` after PR #92 (`0cd3284`). Did not touch Mesh3D pin / Kenney colormap / Walk.wish.

## JPEG pixels (not the G>R mean)

`examples/assets/relic_run/polyhaven/aerial_grass_rock_diff_1k.jpg` is 1024², mean RGB **0.446, 0.381, 0.144**. Almost every texel has R>G, so a "grass = G>R" metric is useless — the moss is yellow-green dry grass.

Decoded with ffmpeg and looked at a 256 preview + tinted preview (`GRASS_TINT` 0.55, 1.55, 0.70):

- Interior: speckled mossy green (after tint: vibrant grass).
- UV 0/1 and the four corners: bare earth. Ring UV 0.00–0.12 has higher B (dirt/rock). That rim is much thicker than 0.035 (~36 px).
- The photo does **not** wrap. Engine sampler is ClampToEdge + Nearest (`make_sampler`). Repeat would still show the dirt frame at every fold.

So #91's pad/blend hypothesis was the right *family* and the wrong *magnitude*. Ping-pong of the full 0..1 range stamps the JPEG's square composition onto the world: a rectangular grass island with an axis-aligned dirt frame, then ClampToEdge stretches the dirt rim across the rest of the peninsula. A 2.6 m UV wobble cannot hide that.

Fallback that looks the same: `heightfield_tile` without `uv_period` defaults `uv_half` to TILE/2, so only world XZ ∈ [−8, 8] has UVs in 0..1 (one 16 m stamp) and everything else is the dirt rim. Crest Isle now also sets `terrain_uv_half = HALF` so that path cannot return.

## Discarded

- Camera-orbit Mesh3D peel (#92). Trees stay textured in Emma's still.
- Missing Kenney grass props. Plants sit on both colors.
- Walk.wish / CharacterController (open PR #93).
- Changing the global sampler to Repeat (would still stamp this photo's dirt frame; new wrap API would be invented).
- Relic Run UV / terrain_base.

## Fix (Crest Isle only)

- `TERRAIN_UV_PAD = 0.28` (≥ `AERIAL_GRASS_DIRT_RIM = 0.12`) so ping-pong stays in the mossy interior.
- `TERRAIN_UV_PERIOD = 9.5` (still ≠ TILE 16). World-continuous; tile centers do not share U.
- `TERRAIN_UV_BLEND = 0` — the 0.018 join wobble was not the rectangle.
- `world.terrain_uv_half = HALF` as a ClampToEdge safety net.

Shared `heightfield_mesh` / Relic Run defaults unchanged.
