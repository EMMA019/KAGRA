# Result — Crest Isle black trees / peel / zoom

## A. Texture lifetime (the peel)

Orbiting must not strip grass/trees to the white 1×1 fallback.

- `kagra-core/src/renderer/gpu_helpers.rs` — `mesh3d_tex_ref_add` / `_sub` / `_pinned`. `ref>0` never evicts.
- `kagra-core/src/renderer/mod.rs` — retained Mesh3D pins bind groups; this-frame is only for immediate draws.
- `kagra-core/src/window.rs` — `upload_mesh_3d` / `unload_mesh_3d` bump `texture_refcount` so GPU pixels match the pin.

White rectangle on the right = fog/camera far/sky (water `skip_fog`, denser outdoor sphere). Not the black squares. Hair is a third system (MToon double-sided left in place; not this LRU).

## Other

- Nature Kit unlit atlas so bark-first pines are not black chrome.
- GLB texture URI from glb dir, not cwd.
- `Walk.zoom_chase` + `[` `]` / `-` `=` / wheel.
- `chunk_decor` denser Kenney variation (existing assets only).

## Play

```bash
python examples/vrm_open_world.py
```

Orbit: grass and Kenney stay textured. `[` `]` zoom the chase cam inside the clamp.

## Verify

```bash
python tools/gen_api_index.py --check
pytest tests -m "not golden"
```

GPU smoke if a `kagra_core` wheel is present.

## Left out

SSAO, CSM, volumetric fog, Rapier, editor, Web/XR, prefetch. Emma merges.
