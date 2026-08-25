# Result — Crest Isle chase cam / meadow / puresky (on #82)

Rebased onto PR #82 (`cursor/crest-isle-tex-bg-561b` @ `0283583`). Cache LRU, path intern, `max_new=1` stream, and leftover-KEYDOWN refresh are **#82's**. This log is the remaining look/camera/sky work plus a 3-frame quiet gap.

## Cause

1. **Re-press:** #81's 16-frame ignore ate Emma's tap. #82 already refreshes a quiet window on leftover KEYDOWN (30 leftover frames stay idle). **15 silent frames (~250ms) still eat a fast re-tap.** After long hold + release, a real same-key press after ~3 quiet frames must walk.
2. **Meadow:** Shared Poly Haven aerial JPEG is brown dirt (mean R>G). Binding the texture was correct; the albedo is not 草原. Tint Crest-only `mesh_mat.base`; do not restore additive IBL.
3. **Sky:** `Stage.draw` disabled fog, queued the sphere, then restored fog **before** the frame flushed. The sky drew fully fogged (not kloofendal puresky).
4. **Camera:** `clip_eye` on overlapping tree AABBs pulled the eye to 5cm from the look-at (inside the skull). Hitch/lerp left a far stale eye (tiny speck). Face white-out / hair backfaces follow from that (N·V < 0 → full fresnel rim). #81 IBL `albedo*0.35` does not flatten front-facing pale skin to a white mask. Far zoom past `fog_end` whites the island.
5. **Black / grey ground:** owned by #82 (bind-group LRU + stream). Not re-solved here.

## Verify

```
python3 tools/gen_api_index.py --check
python3 -m pytest tests -m "not golden" -q
rustup run stable cargo test -p kagra-core --no-default-features --locked input
python3 -m kagra.verify examples/verify_scenarios/open_world_smoke.json
  not run: kagra_core extension is not built in this agent VM
  (`ModuleNotFoundError: kagra.kagra_core`). GPU pixels: CI / Emma's Windows.
```

Did not revert #81 IBL `albedo*0.35`, sun `+Y`, or the sticky-walk leftover KEYDOWN filter. No Rapier. Did not duplicate #82 Mesh3D LRU / path intern / stream `max_new=1`.

## Files

- `kagra-core/src/input.rs` — `REHOLD_QUIET_FRAMES=3` on #82 leftover-refresh
- `kagra-core/src/renderer/{types,mod,shaders}.rs` — skip_fog snapshot / unlit backdrop; MToon backface normal flip
- `kagra/camera3d.py` / `kagra/play.py` — clamp chase distance; freeze `_chase_distance`
- `kagra/look.py` — `mtoon_fill_rgb` CPU stand-in
- `kagra/world3d.py` — `terrain_base` only (stream policy stays #82)
- `examples/vrm_open_world.py` / `open_world_rules.py` — GRASS_TINT, CAM_MIN_DISTANCE, CAM_MAX_DISTANCE
