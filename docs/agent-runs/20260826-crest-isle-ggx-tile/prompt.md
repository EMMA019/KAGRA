Fix Crest Isle remaining ハゲ: one 16 m terrain TILE has dead albedo (only slope GGX), not a hole. Trees sit on it so the heightfield exists. Repo EMMA019/KAGRA, current master (PR #96 merged).

## Emma's diagnosis (verify; do not discard without checking)
World3D._upload_tile passes uv_rect in uv_kw. If the heightfield_tile actually imported at runtime does NOT take uv_rect, TypeError is swallowed by `except Exception: return 0`. Trace the line: one stream tile fails → a black mesh remains (albedo dead, GGX from normals still lighting the slope).

On GitHub master, kagra/gamekit.py heightfield_tile DOES take uv_rect and forwards it. Also check kagra/__init__.py wrappers, any second heightfield_tile, and whether Emma could be running a mixed install (old gamekit + new world3d). Still fix the swallow / leftover-mesh path even if signatures currently match.

## What to trace (this is the job)
1. `_upload_tile` `except Exception: return 0` — silent TypeError / bind failure. Do not leave a GPU mesh in mesh_ids / _tile_meshes with dead albedo. If upload_mesh_3d returns an id but the texture bind is missing or 1×1 fallback, unload that mesh and return 0 so stream_tiles retries (do not mark loaded).
2. Mesh3D bind-group LRU vs shared `_terrain_tex`: a visible streamed terrain tile must not lose its albedo while keeping vertex normals (that is exactly "GGX only"). Pin the terrain albedo for every loaded tile mesh, same family as PR #92 prop pin. Off-camera this frame ≠ unreferenced if the tile is still in _loaded_tiles.
3. LOD upgrade failure: if the new upload fails, do not keep a black replacement; keep the previous good mesh or retry, never a dead-albedo mesh stuck as lod_ok.

GPU-free tests:
- heightfield_tile accepts uv_rect (already) and World3D._upload_tile kwargs match the live signature (inspect.signature test so a drift TypeError cannot return silently).
- Simulated upload failure / zero id / exception does not leave key in _tile_meshes or mesh_ids.
- If you can hook a fake upload that returns an id with a 1×1 / missing tex, that mesh must be unloaded and the tile not sticky-loaded.

## Do NOT
- Revert #95 stream retry/prefetch or #96 TERRAIN_UV_RECT.
- Per-tile local 0..1 UV.
- Rapier / SSAO / Repeat of the uncropped JPEG.
- Merge.
