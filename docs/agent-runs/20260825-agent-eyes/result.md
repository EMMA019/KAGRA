# Result — agent eyes

## APIs

| Call | What |
|---|---|
| `kagra.annotate(sx, sy, cam=, avatar=, world=)` | Click → JSONL (`scratch/annotations.jsonl`) |
| `kagra.debug_trace(foot_y=, height_fn=, on_ground=)` | Over-threshold grounded frames → JSONL |
| `kagra.debug_trace_summary()` | `frames 32-48 floated 0.15` |
| `Camera3D.follow(..., world=)` | Pull eye in before static boxes / trimesh |
| `set_toon_params` | Also steps Prop/terrain Lambert (softness < 0.999) |

## Verify (GPU-free)

```bash
python tools/gen_api_index.py --check
pytest tests -m "not golden" -p tests.no_extension_plugin
```

This checkout: **359 passed, 10 deselected** (`not golden`).

Focused:

```bash
pytest tests/test_annotate.py tests/test_debug_trace.py tests/test_camera3d.py tests/test_physics3d.py tests/test_play.py tests/test_api_index.py -q
```

## Verify (GPU / golden)

```bash
pytest tests/test_golden_render.py::test_pairwise_prop_toon -m golden
```

Switch/Dodge smokes still:

```bash
python -m kagra.verify examples/verify_scenarios/switch_room_smoke.json
python -m kagra.verify examples/verify_scenarios/dodge_room_smoke.json
```

Out of scope (kept out): Tk editor, 4-cascade CSM, SSAO, volumetrics, WebXR, headpat, Rapier, cloth, blend trees.
