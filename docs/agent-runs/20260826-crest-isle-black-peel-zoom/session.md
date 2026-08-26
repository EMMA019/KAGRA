# Session — Crest Isle remaining black trees / peel / chase zoom

Started from `origin/master` after PR #91 (`a27779e`). Emma+review: chase texture lifetime first. Do **not** treat black trees as a missing Kenney material first.

## A. Texture lifetime (black/white peel) — done first

Priority read: `World3D.stream_tiles` → chunk fill (Props stay; tiles unload meshes only) → Prop/terrain `upload_mesh_3d` → Mesh3D bind-group LRU → eviction → draw `fallback_mesh3d_bg` (white 1×1) → GPU `textures` + `texture_refcount`.

Root cause: eviction equated **off-camera this frame** with **unreferenced**. Frustum cull skips a live stream tile / placed Prop, then `MESH3D_TEX_BG_MAX=256` dropped its bind group. Next visible draw missed the BG and sampled Fallback White. Kenney density after that is what fills the 256 slots.

Fix:

- Each `upload_mesh_3d` **pins** `(diffuse, normal)` (`mesh3d_tex_refs`, ref>0 never evict).
- `unload_mesh_3d` drops the pin. `ref==0` is the LRU candidate.
- Hitting 256 must not evict a key still referenced by a live retained mesh (stream tile or Prop). Cache may grow past 256.
- Same pin on `window.texture_refcount` so GPU pixels stay until the last live mesh is gone.
- Immediate draws (water / blob) still count as this-frame live.

Streaming pop-in (B) and prefetch are **not** this fix. LOD still keeps the old mesh until a replacement uploads (no missing-tile hole). White slab on the right is fog/far/background (C), not the black squares. Hair balding is VRM/MToon, not this LRU.

## Black trees (kept, not the peel)

Nature Kit bark-first pines: file-level `KHR_materials_unlit` + `metallic=1` + no albedo. Flatten bakes a 1×N color atlas. Forest Kenney colormap URI still resolves from the **glb directory**, not cwd.

## Zoom

`Walk.zoom_chase` + `[` `]` / `-` `=` / wheel. Clamp unchanged.

## Density (after A)

`chunk_decor` only fills far streamed tiles, so the opening meadow was still sparse. Same PR, already-vendored Kenney only:

- Extra pines / oak / tall / default in the +Z cone (not one cloned tree).
- More rocks, stumps, logs, grass patches on the spawn meadow.
- Ground cover is a 1 m checkerboard of Nature Kit grass / flowers / bushes (jittered).
- `chunk_decor` still mixes pines, oak, grass, flowers, bushes, rocks on streamed tiles.

Blob AO / tile blend / sparks / zoom / texture pin untouched. No Quaternius / Mixamo / Emma.vrm binaries.
