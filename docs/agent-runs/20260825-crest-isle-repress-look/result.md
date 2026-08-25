# Result — Crest Isle chase cam / meadow / puresky (on #82)

Rebased onto PR #82 (`cursor/crest-isle-tex-bg-561b` @ `0283583`). Cache LRU, path intern, `max_new=1` stream, and leftover-KEYDOWN refresh are **#82's**. This log is the remaining look/camera/sky work plus a 3-frame quiet gap.

## Cause

1. **Re-press:** #81's 16-frame ignore ate Emma's tap. #82 already refreshes a quiet window on leftover KEYDOWN (30 leftover frames stay idle). **15 silent frames (~250ms) still eat a fast re-tap.** After long hold + release, a real same-key press after ~3 quiet frames must walk.
2. **Meadow:** Shared Poly Haven aerial JPEG is brown dirt (mean R>G). Binding the texture was correct; the albedo is not 草原. Tint Crest-only `mesh_mat.base`; do not restore additive IBL.
3. **Sky:** `Stage.draw` disabled fog, queued the sphere, then restored fog **before** the frame flushed. The sky drew fully fogged (not kloofendal puresky). First snapshot inferred skip_fog from fog-off and unlit every Mesh3D (CI goldens). Flag is now explicit on sky/backdrop only.
4. **Camera:** `clip_eye` on overlapping tree AABBs pulled the eye to 5cm from the look-at (inside the skull). Hitch/lerp left a far stale eye (tiny speck). Face white-out / hair backfaces follow from that (N·V < 0 → full fresnel rim). #81 IBL `albedo*0.35` does not flatten front-facing pale skin to a white mask. Far zoom past `fog_end` whites the island.
5. **Black / grey ground:** owned by #82 (bind-group LRU + stream). Not re-solved here.

## Verify

```
python3 tools/gen_api_index.py --check
OK /workspace/docs/API_INDEX.md (422 entries)

python3 -m pytest tests -m "not golden" -q
399 passed, 10 deselected

rustup run stable cargo test -p kagra-core --no-default-features --locked input
17 passed (input unit tests)

rustup run stable cargo test -p kagra-core --no-default-features --locked mesh3d_
4 passed (#82 LRU helpers unchanged)

GPU `kagra.verify` was not run here (`kagra_core` not built). CI **golden** failed on the first skip_fog snapshot (fog-off default unlit every Mesh3D). Flag is now opt-in on `sky()` / `Stage.draw` only.

Did not revert #81 IBL `albedo*0.35`, sun `+Y`, or the sticky-walk leftover KEYDOWN filter. No Rapier. Did not duplicate #82 Mesh3D LRU / path intern / stream `max_new=1`.

## Files

- `kagra-core/src/input.rs` — `REHOLD_QUIET_FRAMES=3` on #82 leftover-refresh
- `kagra-core/src/renderer/{types,mod,shaders}.rs` / `window.rs` / `engine/mod.rs` / `kagra/__init__.py` — explicit `skip_fog` on sky/backdrop; MToon backface normal flip
- `kagra/camera3d.py` / `kagra/play.py` — clamp chase distance; freeze `_chase_distance`
- `kagra/look.py` — `mtoon_fill_rgb` CPU stand-in
- `kagra/world3d.py` — `terrain_base` only (stream policy stays #82)
- `examples/vrm_open_world.py` / `open_world_rules.py` — GRASS_TINT, CAM_MIN_DISTANCE, CAM_MAX_DISTANCE
