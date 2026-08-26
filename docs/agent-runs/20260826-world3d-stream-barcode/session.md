Started from `origin/master` after PR #94 (`6843331`). Did not touch Rapier / SSAO / Repeat sampler / per-tile 0..1 UV / Mesh3D pin.

## Screenshot (not a missing mesh)

Emma's remaining-ハゲ still is a **barcode**: whole 16 m tiles in the foreground are 1-axis stretched brown/tan stripes (Nearest ClampToEdge smearing a thin UV sliver). Trees and the VRM are textured. A bright green grass square sits next to yellowish tiles with hard 16 m edges.

Retry-on-failed-upload cannot produce that rectangle. The mesh is there; the UVs are a 1D slice of `aerial_grass_rock_diff_1k.jpg`.

## Hypothesis check (kept)

`heightfield_mesh` ping-pongs `x/period`, `z/period` then insets by `uv_pad`. PR #94 set `TERRAIN_UV_PERIOD=9.5` **smaller** than `TILE=16`, so one chunk spans ~1.68 periods. A `lod_cells=3` triangle is ~5.3 m. On a tile whose interior contains a fold (ox=16), vertex ΔU drops to **0.031** while ΔV stays **0.14** (aspect ~8). GPU interpolates a near-constant U across the triangle → barcode.

`period=48` (3× TILE, multiple of TILE so folds land on chunk edges): every 3-cell triangle on a 16 m tile has ΔU=ΔV≈0.049, aspect 1, UVs still inside pad 0.28 (dirt rim 0.12). Tile centers still do not share U (not per-tile 0..1).

## Streaming bugs (kept, not the barcode)

1. `_loaded_tiles.add` then `_upload_tile` set `_tile_lod` **before** `upload_mesh_3d`. Fail / mid==0 / skip left `lod_ok` true forever. Fix: set lod only on a real mesh id; GPU-ready tiles without a mesh retry. CPU (`_terrain_tex<=0`) still tracks keys for GPU-free tests. After a CPU stream, turning tex on retries upload.
2. Instant unload + `max_new=1` popped tiles. Fix: linger one frame (`_prev_want`); `wanted_tiles` uses `stream_radius + tile` (World3D default, Relic Run included). Sort LOD upgrades by distance **before** brand-new far tiles. Still 1 GPU upload/frame while walking.

## Crest knobs

- `TERRAIN_UV_PERIOD=48` (was 9.5). Pad 0.28, blend 0, `terrain_uv_half=HALF` stay.
- `LOD_CELLS=6` (was 3). Coarser than 8; upgrade-first so a leftover coarse mesh is not preferred over the tile underfoot.

## Discarded

- Per-tile local 0..1 UV (restamps the JPEG square).
- Repeat sampler (dirt rim at every wrap).
- Reverting #94 pad / `terrain_uv_half`.
- New agent APIs.
- Rapier / SSAO.
