# Session — Crest Isle black tile + gold spec

Started from `origin/master` after PR #96 (`0db3a8f`). Did not revert #94 pad, #95 stream retry / period 48 / LOD_CELLS=6, or #96 `TERRAIN_UV_RECT`.

## What the pixels are (not #94/#95/#96)

Emma's remaining patch is **pitch black with a bright gold highlight on the crown of one 16 m hillside tile**. Trees / teal flowers sit on the mesh. Neighbors stay grass.

| Earlier PR | Visual | Cause |
|---|---|---|
| #94 | Bald dirt island / JPEG rim | Full 0..1 ping-pong + ClampToEdge dirt border |
| #95 | 1-axis JPEG barcode / missing rectangle | period 9.5 < TILE + lod_cells=3; failed `upload_mesh_3d` sticky-loaded |
| #96 | Green tile glued to yellowish dirt | Period 48 windows into mixed moss+rock interior; RECT crops meadow |
| this | Black quad + gold GGX streak | PBR (coin metallic=1 / roughness=0.12) on one streamed Lambert tile |

## Investigated and discarded

- **`uv_rect` ignored / throws.** `gamekit.heightfield_tile` / `heightfield_mesh` / public `kagra.heightfield_tile` all take `uv_rect`. Degenerate rect raises. World3D always passes `self.terrain_uv_rect`. GPU-free tests would fail if ignored (UVs would sit in pad `[0.28,0.72]`, not RECT).
- **Hillside UV collapse.** Peak tile `(0,3)` and neighbors, `LOD_CELLS=6` and `CELLS=8`: UV span is a 2D RECT window (aspect ~1), not a 1-axis sliver. `ny > 0.2` (no flipped/zero normals). Tinted JPEG at those UVs has luminance ≫ 0 (not a black texel).
- **Failed GPU upload placeholder.** #95 already refuses to mark a zero/`Exception` upload as loaded. That is a **hole** (sky/clear `cls(150,175,195)`), not a shaded black quad with spec. Fallback Mesh3D BG is **white** 1×1 (orbit peel), not black.
- **Shadow AABB skip.** `SHADOW_SKIP_EXTENT=24`. A 16 m tile with hillside Y span still samples as lit when outside the map (factor 1.0), not black. Spot-own shadow dark floor is 0.16 × grass, still green.
- **Brighter `GRASS_TINT`.** Not done. Tint is already Crest-only meadow multiply.

## Remaining cause

Lambert terrain (`metallic=0`, `roughness=1`) and gold coins (`1.0` / `0.12`) share the Mesh3D `mesh_mat` dynamic uniform buffer.

1. **Pack vs draw desync.** `draw_meshes_3d` built `retained_mats` with `filter_map` (drop missing id) then drew `visible_retained` with `slot++` and `continue`. One dropped mesh shifted later tiles onto the **previous frame's instance-pass leftover** (coin PBR). Dielectric/metal GGX + dark albedo = pitch black + gold streak on the slope crown. Bloom (`threshold=0.80`) sells the highlight.
2. **Grown `mesh_mat` buffer was uninitialized.** `ensure_mesh_mat_slots` allocated a new UNIFORM buffer and only wrote used slots. An unwritten slot is undefined GPU memory — same visual.
3. **Draw listed `mesh_ids`, not `_tile_meshes`.** A live tile missing from `mesh_ids` was skipped; extras could still be queued.

World3D also omitted explicit `metallic` / `roughness` on `upload_mesh_3d` (Python defaults 0/1, but nothing pinned the retained mesh against `set_mesh_pbr` / a default PBR slot).

## Fix (public APIs only)

- `World3D._upload_tile`: `upload_mesh_3d(..., metallic=0, roughness=1, base_color=terrain_base)` then `set_mesh_pbr` with the same Lambert values.
- `World3D.draw`: emit live `_tile_meshes`, then non-box `mesh_ids` that are not those tiles (Overworld ramp still draws).
- Renderer (surgical, not a rewrite): Lambert-init every mesh_mat slot on create/grow; pack one material per visible retained id (missing → Lambert, list length matches); bind slot from index, not a running counter that skips on `continue`.

Relic Run does not set `terrain_uv_*` / `set_mesh_pbr` on terrain. Coin PBR unchanged.

## Not done

- Repeat sampler / per-tile 0..1 UV / second JPEG.
- Rapier, SSAO, Tk editor, 2D ECS, renderer rewrite.
- GPU `kagra.verify` in this environment (no `kagra_core` wheel). GPU-free pytest is the close-the-loop here; smoke JSON has a notes field.
