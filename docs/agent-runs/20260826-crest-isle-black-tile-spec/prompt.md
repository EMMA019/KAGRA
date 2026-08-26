Fix the Crest Isle terrain "bald/peeling" tile in EMMA019/KAGRA (repo https://github.com/EMMA019/KAGRA, branch master).

USER BUG: Running `python examples/vrm_open_world.py` (window title "VRM Crest Isle"), a large rectangular pitch-black patch appears on a hillside. Sharp tile-shaped edges. Geometry is still there (trees and teal flowers sit on the black quad). A bright golden specular highlight sits on the TOP of the black patch, so this is not a hole in the mesh — albedo/material/UVs/lighting on ONE streamed heightfield tile is wrong. Rest of the terrain is normal grass/dirt.

CONTEXT (do not treat as ground truth — verify in code):
- World3D streams 16m tiles (`TILE=16`, `STREAM_RADIUS=64`, `LOD_RADIUS=28`, `LOD_CELLS=6`, `CELLS=8`) in `kagra/world3d.py`.
- Crest UV constants live in `examples/open_world_rules.py`: `TERRAIN_UV_PERIOD=48`, `TERRAIN_UV_PAD=0.28`, `TERRAIN_UV_RECT=(0.535, 0.485, 0.640, 0.590)`, `GRASS_TINT=(0.55, 1.55, 0.70)`.
- A recent commit already tried: (1) do not mark a tile loaded if `upload_mesh_3d` fails ("bald rectangle"), (2) bump period/lod_cells so 16m chunks are not a 1D UV barcode. The user still sees a black tile WITH specular, so that fix is incomplete or a different bug.
- Sampler is ClampToEdge + Nearest. JPEG `aerial_grass_rock_diff_1k.jpg` has a dirt rim; they crop with RECT.
- `world3d._upload_tile` passes `uv_rect=self.terrain_uv_rect` into `heightfield_tile`. Confirm `kagra/gamekit.py` actually accepts and applies `uv_rect`. If the kwarg is ignored or throws, that is a bug.
- Streaming is 1 new tile per frame after warmup. LOD upgrade used to unload first (missing rectangles); current code tries to keep old LOD. Verify draw path actually draws `_tile_meshes` and does not leave a default black mesh.
- Coins use metallic=1 roughness=0.12. Terrain should NOT pick up that PBR. Check whether `set_mesh_pbr` / default mesh material can make a terrain tile black+specular.
- Strong hypotheses (non-binding, investigate and keep or discard): missing/black albedo bind on one tile; UVs collapsing to a black texel for that chunk; flipped/zero normals on one heightfield tile (unlit + GGX rim on the slope); `uv_rect` not applied in gamekit; failed GPU upload still drawing a placeholder; shadow AABB skipping the tile so only specular remains; `terrain_base` / tint producing black under some lighting.

DO:
1. Trace World3D.draw / mesh_ids / _tile_meshes and the wgpu Mesh3D bind (texture + material) for streamed tiles.
2. Trace heightfield UV generation vs TERRAIN_UV_RECT/period/pad. Add a GPU-free unit test if UV for a TILE-sized chunk is degenerate (zero area, 1-axis sliver, or outside the meadow rect).
3. Fix the actual cause. Do not paper over with a brighter tint.
4. Do not expand scope to the Tk editor or 2D ECS. Do not rewrite the renderer.
5. Keep Crest Isle on public APIs. Add/adjust tests under tests/ (physics-free / GPU-free preferred). If you can, add a verify scenario note.
6. Open a PR with a clear explanation of root cause vs the earlier bald-tile/barcode fix.

Success: one streamed hillside tile no longer renders as a black quad with a gold specular streak while neighbors stay grass. Trees sitting on that tile still sit on the mesh.
