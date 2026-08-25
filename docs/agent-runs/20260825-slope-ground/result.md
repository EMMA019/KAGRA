# Result — slope grounding (tight foot AABB)

## What changed

| Piece | What |
|---|---|
| `kagra.physics3d.FOOT_RADIUS` | 0.08 m foot ring (wall capsule stays ~0.28) |
| `GROUNDED_FLOAT` | 0.05 — documented `\|foot_y − terrain\|` while `on_ground` |
| `height_support` | extra samples + snap-to-plane; `snap_to_plane=False` is the fat AABB |
| `World3D.update` | feeds `debug_trace` when a tracer is active |
| Rapier | **not added** (5MB wheel still a separate decision) |

## Verify (GPU-free)

```bash
python tools/gen_api_index.py --check
pytest tests/test_physics3d.py tests/test_debug_trace.py tests/test_world3d.py -q
pytest tests -m "not golden"
This checkout: **368 passed, 10 deselected** (`pytest tests -m "not golden"`).

Focused:

```bash
pytest tests/test_physics3d.py tests/test_debug_trace.py tests/test_world3d.py tests/test_api_index.py -q
python tools/gen_api_index.py --check
```

## Out of this PR

Rapier crate, cloth, blend trees, spatial audio, multi-avatar, visual editor, CSM / SSAO / WebXR, `kagra.stage` unshadow (#78).
