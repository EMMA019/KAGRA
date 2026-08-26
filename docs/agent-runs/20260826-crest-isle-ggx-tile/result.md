# Result — Crest Isle GGX-only TILE

## Cause

One streamed 16 m tile could keep a GPU mesh with vertex normals (slope GGX) after albedo bind failed or fell back to 1×1. `_upload_tile`'s `except Exception: return 0` did not unload that id. A failed LOD upgrade could also replace a good mesh with a dead one and stick `lod_ok`. Signatures on master already include `uv_rect` (mixed-install TypeError is a hole, not this symptom).

## Files

- `kagra/world3d.py` — signature-safe UV kwargs, dead-albedo unload/retry, keep previous LOD mesh
- `kagra-core/src/renderer/mod.rs` — bind group at `upload_mesh_3d`; culled retained tiles stay in the ensure set
- Tests: `tests/test_world3d.py`, `tests/test_open_world.py` pin source scan

`#95` stream retry / prefetch and `#96` `TERRAIN_UV_RECT` untouched. Relic Run UV defaults unchanged.

## Verify

```
python3 -m pytest tests -m "not golden"
```
