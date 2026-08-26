Fix remaining Crest Isle "bald ground" (ハゲ). Repo EMMA019/KAGRA, start from current master (PR #92 texture-pin already merged).

## What Emma just reported
Black Kenney trees are FIXED after #92. Remaining bug: the meadow still looks bald. Attached screenshot is her latest still (after #92).

What the shot shows:
- Trees: teal Kenney foliage + brown trunks. Good. Do not reopen colormap/URI/LRU pin.
- Ground: a rectangular island of vibrant speckled green grass (lower-left) with a perfectly axis-aligned hard edge, then yellowish-brown dirt for most of the rest of the island. Small Kenney plants/flowers sit on BOTH the green and the brown, so this is the HEIGHTFIELD albedo, not missing grass props.
- Character (orange hair, white/purple) on a blue ground ring. Fog/grey sky as before.

This is NOT camera-orbit Mesh3D peel (trees stay textured). It is the terrain grass/dirt rectangle.

## Context already on master (verify; do not assume this is the whole cause)
PR #91 added Crest Isle-only UV knobs because the meadow JPEG is ClampToEdge and UVs outside 0..1 sample the JPEG dirt border:
- examples/open_world_rules.py: TERRAIN_UV_PERIOD=13.5, TERRAIN_UV_BLEND=2.6, TERRAIN_UV_PAD=0.035
- Comment there: aerial_grass_rock_diff_1k.jpg mean is brown dirt (AERIAL_GRASS_ALBEDO ~ 0.446, 0.381, 0.143); GRASS_TINT (0.55, 1.55, 0.70) tries to read as 草原
- kagra/gamekit.py heightfield_mesh / heightfield_tile: uv_period ping-pong, uv_blend, uv_pad
- kagra/world3d.py wires terrain_uv_period / uv_blend / uv_pad
- tests/test_gamekit.py has test_heightfield_uv_blend_is_not_a_step_at_the_join
- Agent log: docs/agent-runs/20260826-crest-isle-trees-ao-seams-sparks/session.md

Hypothesis (non-binding; investigate and discard if wrong): the 2.6 m blend + 0.035 pad did not hide ClampToEdge sampling of the JPEG's dirt rim, so ping-pong still paints a rectangular grass patch vs dirt. Alternative: the JPEG itself is grass-in-center / dirt-at-edges and pad is too small. Alternative: sampler should Repeat and UVs should stay inside the grassy texel region. Look at the actual JPEG pixels before changing constants.

## Goal
Crest Isle meadow should look continuously grassy (or smoothly blended grass/dirt with no axis-aligned rectangular grass island). Relic Run / other World3D demos keep default UV. Do not add Rapier, SSAO, volumetric, editor, networking. Do not rewrite CharacterController / Walk.wish (that is open PR #93 on another branch; leave it alone). Do not revert #92 pin/refcount LRU or zoom keys.

## How to tell it's done
- GPU-free tests: pytest tests -m "not golden" (or the project's equivalent). Add/extend a test that adjacent stream tiles do not produce a UV/albedo step the size of TILE, and that Crest Isle UVs do not sample the JPEG dirt border.
- If you can inspect the meadow JPEG, document why grass vs dirt appears as a rectangle.
- Open a PR. Do not merge.
- Short log under docs/agent-runs/.
- Keep agent APIs existing; do not invent new ones. After visual engine changes, run kagra.verify if a GPU wheel exists; if not, note it.

Investigate first. Share hunches only as hypotheses. Fix the real cause.
