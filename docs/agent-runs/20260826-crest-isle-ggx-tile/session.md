# Session — Crest Isle remaining ハゲ (GGX-only TILE)

Started from `origin/master` after PR #96 (`0db3a8f`). Did not touch stream retry / prefetch / delayed unload / `TERRAIN_UV_RECT` / per-tile UV / Rapier / SSAO.

## Emma's TypeError line (checked, not discarded)

On this master, `kagra.gamekit.heightfield_tile` / `heightfield_mesh` **and** `kagra/__init__.py` wrappers all take `uv_rect` and forward it. `World3D` imports `heightfield_tile` from `gamekit`, not the package wrapper. There is no second implementation.

A mixed install (old gamekit + new world3d) would still TypeError on `uv_rect=`. `#95` already avoids marking that tile `lod_ok` when upload returns 0, so a pure TypeError before `upload_mesh_3d` is a **hole**, not GGX-only. Trees can still sit on it: collision is CPU `height_fn`, independent of the GPU mesh.

The GGX-only rectangle is a **leftover GPU mesh**: vertex normals light the slope, albedo is Mesh3D Fallback White 1×1 (or a 1×1 / missing bind). That happens when:

1. `upload_mesh_3d` returns an id after the bind group is missing, and `_upload_tile` stores it.
2. LOD upgrade unloads the previous good mesh and keeps the dead replacement as `lod_ok`.
3. Bind group is created only on first **visible** draw; a retained stream tile that is off-camera this frame can miss the BG even though PR #92 already pins `(diffuse, normal)` on retain.

## Fix

- `_uv_kwargs_for`: `inspect.signature` against the live `heightfield_*`. Unknown keys raise (CI tripwire) instead of a swallowed TypeError after a partial upload. Exception path unloads any leftover id.
- After upload, `texture_size` of `_terrain_tex`: missing or 1×1 → `unload_mesh_3d`, return 0, do not mark loaded. Engine-not-running (`_check()`) is not treated as dead albedo (GPU-free tests).
- LOD upgrade: return 0 without writing `_tile_meshes` / `_tile_lod`, so the previous good mesh stays and stream retries.
- Rust: `upload_mesh_3d` calls `ensure_mesh3d_tex_bg` immediately; if `texture_id != 0` and the BG was not created, unload and return 0. Draw passes also pass **all** retained Mesh3D tex keys (culled stream tiles), not only this-frame visible.

## Discarded

- Reverting #95 / #96.
- Per-tile local 0..1 UV / Repeat JPEG.
- Rapier / SSAO.
- New public agent APIs (`texture_size` already exists).
