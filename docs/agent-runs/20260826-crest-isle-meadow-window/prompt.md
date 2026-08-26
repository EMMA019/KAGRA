Fix remaining Crest Isle meadow ハゲ after PR #95. Repo EMMA019/KAGRA, start from current master (PR #95 already merged).

## Screenshot (attached) — this is AFTER #95
Barcode / 1-axis stretch is GONE. What remains: a sharp straight seam (tile edge, looks diagonal on the hill) between lush green grass on the left and yellowish-brown dirt on the right where the character stands. Trees and Kenney plants are textured on BOTH sides. Blob AO under the VRM. Fog as usual. Both sides look blocky (Nearest).

Emma: 相変わらずはげてはいる. The brown side is the ハゲ.

## Cause to verify (do not assume)
`aerial_grass_rock_diff_1k.jpg` is a mixed aerial photo (moss + dirt/rock even inside the pad 0.28 interior). PR #95 set TERRAIN_UV_PERIOD=48 so each 16 m TILE is a 2D window into that photo. Adjacent tiles therefore sample different biomes of the same JPEG → hard grass vs dirt chunk edge. Not Mesh3D peel. Not missing mesh. Not period 9.5 barcode.

Inspect the JPEG pixels (and after GRASS_TINT 0.55, 1.55, 0.70). Find a compact region that stays meadow-green (not dirt). Map Crest Isle UVs into that window only, world-continuous, period still significantly larger than TILE, pad/window must never sample dirt rim or brown rock patches. Relic Run keeps the uncropped JPEG / default UV.

## Do NOT
- Revert #95 stream retry / prefetch / delayed unload / LOD upgrade order / LOD_CELLS=6 / period>TILE.
- Per-tile local 0..1 UV (restamps the JPEG square).
- Repeat of the uncropped photo.
- Rapier, SSAO, new agent APIs.
- Merge the PR.

## Done when
- Adjacent 16 m Crest tiles do not jump from green meadow to bald dirt (GPU-free test: sample tinted JPEG at Crest UVs across a 3×3 tile neighborhood; green channel stays meadow-like, no dirt-rim / brown-rock step at TILE boundaries).
- pytest tests -m "not golden" green.
- PR open. Short log under docs/agent-runs/.

Investigate the JPEG first. The screenshot is the acceptance test: the hill should read as continuous 草原, not a green tile glued to a brown tile.
