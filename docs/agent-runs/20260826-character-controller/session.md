# Session — 2026-08-26 character controller (AABB, no Rapier)

Master at start: `a27779e` (#91 trees/AO/orbs). Did not touch leftover-black-trees work (stream tiles, Kenney density, camera peel/zoom). Crest Isle only swapped `Walk(..., controller=CharacterController(...))`.

## API search

- `Walk.update` overwrote `player.vx/vz` every frame from `walk_wish` (instant start/stop). Sticky-walk quiet gap 3 is `kagra-core` `REHOLD_QUIET_FRAMES` — left alone.
- Slope sit: `FOOT_RADIUS=0.08`, 4 cardinal samples, `snap_to_plane`. Fat AABB max-Y still floats (existing tests).
- `height_support` raised sit by raw max-Y when a ring sample beat the tangent plane by `1e-4` — slope curvature / a 6 cm terrace 8 cm uphill leaked the fat-AABB lift through the "bump" exception.
- Ground collision snapped **up** only (`y < gy`). A 2–5 cm hover stayed airborne until gravity dropped it (floaty).
- Heightfield stairs climb via `step_height`. Static prop lips did not: capsule vs AABB slid the wall.
- Capsule ground friction damped ~13%/frame, which would fight a motor if we stopped overwriting vx.

No Rapier crate in the tree. `rapier3d` + nalgebra is the bulky path; AABB already had capsule vs AABB/OBB/trimesh.

## Hypothesis (measured, kept)

Slope float is **not** missing Rapier. Tight foot + plane snap already keeps linear grade 0.4 under `GROUNDED_FLOAT` 0.05. Remaining cheap feel:

| Cause | Evidence |
|---|---|
| One-sided max-Y bump raise | 6 cm shelf 8 cm uphill: raw max-Y sit +0.09 m (> 0.05). Plane-priority keeps it. |
| 4 cardinals | Diagonal pebble at (0.056, 0.056) missed. 8-point ring hits. |
| Snap-up only | 4 cm hover on a slope stayed off-ground. Stick-down ≤ 0.05 glues it. |
| Instant vx | `move_player(wish)` every frame. Accel 14 / decel 22. |
| No box step-up | 0.30 m crate was a wall. `_try_step_up` if rise ≤ `step_height`. |

Discarded Rapier for this PR. Wheel size unchanged (no new crate).

## Decisions

- New `kagra/controller.py`: `CharacterController.wish` / `move` / `try_jump` / `apply`, `accelerate_xz`. GPU-free.
- `Walk` owns a controller (defaults accel=14, decel=22). Agents: `Walk.wish` / `Walk.move` / `Walk.try_jump`.
- Crest Isle thin swap: pass `controller=kagra.CharacterController(speed=..., jump=..., accel=14, decel=22)`.
- Physics: 8-point foot, `BUMP_PLANE_ERR=0.08`, `GROUND_STICK=0.05` (not while `vy>0.25`), capsule skips friction, step-up vs static solids.
- Input `REHOLD_QUIET_FRAMES=3` untouched.

## Stumbles

- `Walk.jump` is already the impulse height, so the method is `try_jump` (same pattern as `jump_vy`).
- Idle `vx=0` after `world.update` would hide decel. Now only snaps when hypot < `IDLE_SNAP` (0.12) and not steep-sliding.
- Skipping friction on all capsules made a trimesh ramp slide through (`test_capsule_does_not_fall_through_ramp`). Friction skip is only when `body.controlled` (set by `apply`).
- Step-up used trimesh AABB max-Y as a kerb and launched over ramps. Trimesh is skipped; heightfield stairs still use `step_height`.
- First `apply` on an already-grounded body must not count as `landed`.
- `Walk.update` + stub `kagra` had no `get_engine`; wrap it. Dummy `object()` cam still cannot `follow` — tests use `Camera3D`.
- Did not rewrite `vrm_open_world.py` beyond the Walk constructor / one docstring line.
