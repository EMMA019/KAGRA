# Session — Crest Isle meadow window (remaining ハゲ after #95)

Started from `origin/master` after PR #95 (`a2ba4de`). Did not touch stream retry / prefetch / delayed unload / LOD upgrade order / `LOD_CELLS=6` / `TERRAIN_UV_PERIOD=48`.

## JPEG pixels (tinted)

`examples/assets/relic_run/polyhaven/aerial_grass_rock_diff_1k.jpg` is 1024² baseline JPEG, mean RGB **0.446, 0.381, 0.144**. After `GRASS_TINT` (0.55, 1.55, 0.70) almost every texel is G-dominant, so G>R is useless. The dirt tell is **blue / yellow**: mossy meadow has low B (~0.02–0.04 tinted); dirt/rock and the square rim have tinted B ≳ 0.10 (yellowish-brown ハゲ).

16×16 block map of tinted B / dirt-frac (`tB>0.10`):

- Square rim and several interior patches are dirt (frac 0.7–1.0).
- Pad 0.28 window UV ∈ [0.28, 0.72] still has dirt frac **0.29**.
- With period 48, tile (0,0) mean tinted B=0.070 vs tile (1,0) B=0.106 (dirt frac 0.27 vs 0.51). That is Emma's seam: a green 16 m chunk glued to a bald one. Join UVs match (world-continuous); the *interiors* sample different biomes.

Compact meadow (low B, dirt frac ~0.03, inside rim 0.12 and pad 0.28): UV **(0.535, 0.485)–(0.640, 0.590)**. 3×3 TILE neighborhood: tinted B 0.010..0.029 (step 0.019), G 0.558..0.601, max dirt 0.07. Join mean B is continuous.

## Discarded

- Camera-orbit Mesh3D peel / missing Kenney (trees+plants textured on both sides of the seam).
- Period 9.5 barcode (#95 already gone in the screenshot).
- Raising pad toward the image center (center is still mixed; pad is a centered inset, not an offset window).
- Per-tile 0..1 UV / Repeat of the uncropped photo.
- Cropping a second JPEG for Crest (Relic Run keeps the same file; UVs pick the window).
- Rapier / SSAO / new agent callables.

## Fix (Crest Isle only)

- `heightfield_mesh` / `heightfield_tile` / `World3D`: optional `uv_rect=(u0,v0,u1,v1)`. Ping-pong 0..1 maps into that window. Unset → existing pad / 0..1 (Relic Run).
- `TERRAIN_UV_RECT = (0.535, 0.485, 0.640, 0.590)`. Period 48, pad 0.28, blend 0, `LOD_CELLS=6` unchanged.
- GPU-free test samples the tinted JPEG (ffmpeg 1K, or `tests/data/aerial_grass_rock_128.rgb.z` box-average) across a 3×3 TILE neighborhood.

## CI

GitHub on PR #96 head `1adc793`: **17 checks green**.
