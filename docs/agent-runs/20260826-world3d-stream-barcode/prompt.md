Harden World3D terrain streaming. Repo EMMA019/KAGRA, start from current master (PR #94 bald-meadow UV already merged).

## Do NOT do
- Do not switch tiles to per-chunk local UV (uv_half=None / 0..1 per tile). World-continuous UV is the intended design; Crest Isle TERRAIN_UV_PERIOD / PAD stay.
- Do not revert #94 (pad 0.28, period 9.5, terrain_uv_half=HALF).
- Do not add Rapier, SSAO, Repeat sampler for this JPEG (it has a dirt rim; Repeat would stamp the rim).
- Do not invent new agent APIs.

## Real bugs (verified on master kagra/world3d.py)

1) Failed upload stuck as loaded (must fix).
`stream_tiles` does `_loaded_tiles.add(key)` THEN `_upload_tile`. `_upload_tile` sets `_tile_lod[key]=cells` BEFORE the try, then on exception / mid==0 / `_terrain_tex<=0` returns 0. Next frame `lod_ok = have and lod==want_cells` skips forever. A missing mesh stays a bald rectangle.
Fix: only add to `_loaded_tiles` and `_tile_lod` when `upload_mesh_3d` actually returns a mesh id. Failed or zero upload must leave the key unloaded so the next `stream_tiles` retries. GPU-free test: fake a failing upload (or `_terrain_tex=0` after marking want) and assert the key is not sticky-loaded and is retried.

2) Immediate unload + 1 new tile/frame (should fix, keep it small).
`stream_tiles` unloads any key not in `want` immediately, then adds at most 1 new tile while walking (`max_new=1` after warm). Fast camera/player motion shows missing tiles (pop-in / ハゲ amplifier). Do not remove the 1/frame budget for brand-new GPU uploads (that's the hitch guard). Instead:
- Delayed unload: keep a tile one extra frame (or until its replacement exists) instead of instant `_unload_tile`.
- Prefetch: `wanted_tiles` / stream radius should include a 1-tile ring beyond the visible set (or along last move delta) so the next chunk is already loading before it hits the camera.
GPU-free tests: moving the viewer so tile A leaves want does not unload A in the same call if it was visible last frame; a viewer step toward +X requests the +X neighbor before it is the only visible new tile.

Relic Run / non-streaming worlds must stay default. Crest Isle `vrm_open_world.py` UV knobs unchanged except if they must opt into the new prefetch radius (prefer World3D default so every stream_radius world gets it).

## Done when
- pytest tests -m "not golden" green, including new stream retry / delayed-unload tests.
- PR open, do not merge.
- Short log under docs/agent-runs/.
- CHANGELOG one bullet.

Investigate first. The failed-upload order is the confirmed bug; prefetch/delay is the streaming design fix. Discard extra scope.

---

Follow-up (Emma screenshot):

Emma just sent the actual remaining-ハゲ screenshot (attached). Primary visual is NOT a missing mesh. It is BARCODE / 1-axis stretch on whole 16 m terrain tiles in the foreground (long parallel brown/tan stripes = one JPEG texel row stretched across the chunk). Trees and the VRM are fine. A bright green grass square sits next to yellowish tiles with hard edges.

Retarget. Keep the failed-upload sticky-loaded fix. Prefetch/delayed unload still good. But the screenshot will NOT be fixed by retry alone.

Hypothesis (verify; discard if wrong): PR #94 set TERRAIN_UV_PERIOD=9.5 which is SMALLER than TILE 16. Ping-pong of the whole moss window therefore happens INSIDE one tile. With lod_cells=3 (and Nearest ClampToEdge), a triangle spans a long 1D slice of aerial_grass_rock_diff_1k.jpg → barcode. Do not switch to per-tile local 0..1 UV (that restamps the JPEG square). Prefer world-continuous UVs where one TILE maps to a SMALL 2D window of the moss interior (period significantly larger than TILE, keep pad so UV 0/1 dirt rim is never sampled). Also do not leave a 3-cell LOD mesh in the camera foreground: either raise min cells so ΔUV per triangle is 2D and small, or upgrade LOD before new far tiles, or both.

GPU-free tests should fail if a 16 m tile + Crest knobs + lod_cells=3 maps a triangle across a near-zero ΔU or ΔV (barcode) or across the JPEG dirt rim. Relic Run defaults unchanged. No Rapier. No Repeat of the uncropped photo. Do not merge.

Look at the attached shot. That barcode rectangle is the acceptance test.
