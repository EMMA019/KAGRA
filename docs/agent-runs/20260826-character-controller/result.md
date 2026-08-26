# Result — character controller (AABB, no Rapier)

## What changed

| Piece | What |
|---|---|
| `kagra.controller.CharacterController` | wish / move / try_jump / apply. Accel 14, decel 22, air control 0.38 |
| `Walk.wish` / `Walk.move` / `Walk.try_jump` | Agent-facing motor. `Walk(..., controller=)` |
| `height_support` | 8-point ring. Bump raise only if sample beats plane by 0.08 m |
| `GROUND_STICK` | Snap down ≤ 0.05 m while not jumping |
| Step-up | Capsule vs static prop lip ≤ `step_height` |
| Capsule friction | Skipped only when `body.controlled` (motor owns XZ stop) |
| Crest Isle | Thin `Walk(..., controller=CharacterController(...))` |
| Rapier | **not added** (AABB was enough). Wheel size: no crate impact |

Sticky-walk quiet gap 3 (`kagra-core` input) unchanged. Mixamo bind, spatial audio, blob AO, stream tiles, camera zoom untouched.

## Verify (GPU-free)

```bash
python3 tools/gen_api_index.py --check
python3 -m pytest tests/test_controller.py tests/test_physics3d.py tests/test_debug_trace.py tests/test_play.py tests/test_public_names.py tests/test_api_index.py tests/test_world3d.py -q
python3 -m pytest tests -m "not golden" -p tests.no_extension_plugin
```

This checkout: **462 passed, 10 deselected**.

Focused: accel ramps then stops, Walk.wish/move/try_jump, jump+land, slope stand under `GROUNDED_FLOAT`, one-sided terrace does not fat-lift, diagonal pebble sampled, crate step-up, tall wall still blocks, fat AABB still floats (hypothesis check), sticky-walk idle path unchanged.

**GitHub CI: 17 checks passed** on `461a500` (including `golden`, Windows/macOS/Linux wheels, python-unit 3.10–3.12). Cursor Bugbot was usage-limited (neutral). No Rapier crate.

## Out of this PR

Rapier crate, NavMesh, SSAO, CSM, volumetric, visual editor, networking, quests, inventory. leftover-black-trees camera peel / Kenney density / stream tiles.
