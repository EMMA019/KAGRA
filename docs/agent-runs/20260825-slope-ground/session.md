# Session — 2026-08-25 slope grounding (tight foot AABB)

Master at start: `9868bde` (#77 Crest Isle `_chunk_props` + #76 `debug_trace`). Did not redo #78 (`kagra.stage` unshadow). Did not touch `kagra/__init__.py`.

## API search

- `Physics3D._ground_collision` sampled `height_fn(x, z)` at the capsule **center** only.
- Walk / Crest Isle capsule wall radius is **0.28**. The AABB footprint is 0.56 m.
- `height_normal` + slope-slide already exist. No Rapier crate in the tree (on purpose).
- `kagra.debug_trace` / `debug_trace_summary` already public. `World3D.update` did not feed them.

## Hypothesis (measured, kept)

Center-only snap keeps `|body.y − height_fn(center)| ≈ 0`, so `debug_trace` was quiet even while the **fat** capsule AABB sat above the downhill side of a slope.

On grade 0.4 with radius 0.28:

| Probe | Δ |
|---|---|
| center sample (old sit) | ~0.00 |
| max-Y under fat AABB (0.28) | **0.11** (> 0.05) |
| max-Y under tight foot (0.08) | 0.03 |
| snap-to-plane extra at 0.08 | ~0.006 |

So the float Emma would see is the fat box, not a missing Rapier solver. Discarded “need Rapier in this PR”.

## Decisions

- `FOOT_RADIUS = 0.08` (wall capsule stays 0.28). Extra samples: center + 4 axes. Cliffs beside the foot (`step_height` / `max_grade`) are skipped so a wall does not launch the body.
- `snap_to_plane=True` (default): project the highest *walkable* sample’s tangent plane back to the capsule center. That is what stops `grade * radius` lift.
- `snap_to_plane=False` + `foot_radius = capsule.radius` is the loosened fat AABB — `debug_trace` then emits.
- Documented budget: `GROUNDED_FLOAT = 0.05` (same as `debug_trace` default). Applies while `on_ground` on a **walkable** slope. Steep slide is already a different path.
- `World3D.update` samples `debug_trace` only when `kagra.trace._ACTIVE` is set. Crest Isle / Relic Run / Overworld all go through that. No JSONL spam in normal play.
- Still no Rapier. Measure first.

## Stumbles

- Wiring `debug_trace` from `Walk` would double-count with `World3D.update`. Only the world ticks.
- Extra samples without a cliff filter would snap the player onto a 5 m wall 8 cm to the side.
- `test_stairs_are_climbable` asserted `y>1.2` after 80 frames at vz=3. That is after walking **off** the last step (z>3.2) while falling. Tight foot keeps `on_ground` a little longer so hang-time is ~1 cm lower (1.19 vs 1.35). The climb itself still reaches y≈1.88 on the stairs. The test now checks peak Y on the stair, not airborne height after the drop.
- `kagra/__init__.py` left alone so #78 can land; `stage` stays whatever master/PR #78 make it.
