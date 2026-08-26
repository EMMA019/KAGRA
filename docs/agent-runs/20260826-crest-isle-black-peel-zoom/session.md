# Session — Crest Isle remaining black trees / peel / chase zoom

Started from `origin/master` after PR #91 (`a27779e`). Did not stop at "colormap URI should have fixed it."

## Black trees (confirmed)

Flattened every tree path in `VISTA_PROPS` + `chunk_decor`:

- `forest/tree.glb`, `forest/tree-high.glb`, `town/tree-crooked.glb`, `castle/tree-*` already load sibling `Textures/colormap.png` (`metallic=0`). #91's URI + KHR UV path is fine for these **if** resolve uses the glb directory, not cwd.
- Nature Kit `tree_pineTallA` / `tree_default` / `tree_palm` / `tree_oak` / `tree_tall` have **no images**, `KHR_materials_unlit` on **file** `extensionsUsed` only (not on each material), `metallicFactor: 1`, per-primitive `baseColorFactor` (bark brown vs mint foliage).
- `flatten_gltf` took PBR from the **first primitive**. Bark-first pines (`pineTallA`, `tree_default`, `tree_palm`) became metallic=1 with no albedo → black chrome silhouettes. Foliage-first `pineTallB` looked mint (Emma: "Nature Kit pines look OK").
- Vista matches her shot: palms at x≈-11/-10/-9, `tree_default` at `(1.6, 10.2)`, `pineTallA` further back.

Fix: treat file-level `KHR_materials_unlit` as unlit; bake a 1×N `baseColorFactor` atlas (Nearest-safe UV `(i+0.5)/n`) when a GLB has 2+ untextured colors; force `metallic=0` / `roughness=1`. Do **not** treat untextured `metallic>0.5` as unlit (that would zero a real chrome material; `test_flatten_reads_pbr_factors` stays 0.7). URI resolve: `_gltf_dir` + `Path.resolve()` against the glb parent, percent-decode, reject `..`. Cwd is never consulted.

## Peel / はげる (several causes, not one)

1. **Missing-tile rectangles:** `stream_tiles` unloaded LOD-stale tiles first, then re-uploaded with `max_new=1`. Camera+walk left dark holes / sky showing as a white slab until the budget caught up. Now: keep the old LOD mesh; upload replacement then unload the old id. `_upload_tile` had an indent bug (assignment sat under `if not mid: return 0`) — fixed so GPU ids actually stick.
2. **White fog rectangle:** `water(..., half=80)` was fogged (huge quad reads as a screen-aligned slab). `skip_fog=True`. Outdoor puresky used `backdrop_sphere` default 16×24 → one huge triangle covering half the view when orbiting; `radius>=40` now uses rings=32, segs=48.
3. **Stretched grass / dropping Kenney:** Mesh3D LRU 256. Texture-exists-only still failed if `live` was the fallback 1×1; this-frame-only would drop grass when a later Kenney pass filled 256 slots. Evict only when **neither** this-frame **nor** the diffuse texture is loaded. Morph BG cache was FIFO 128 and skipped draws with `None` → VRM hair went bald; now `lru_evict_dead` with this-frame morph keys.
4. **Hair cards vanish behind the head:** single-sided cull. Hair materials (VRM0 + VRM1 name match) force `double_sided`. Face stays authored.

## Zoom

`Walk.distance` is hitch-protected (mutating the public field does not move the arm). Added `Walk.zoom_chase(delta)` → `clamp_chase_arm` (scale horizontal arm, keep pitch, clamp 3D hypot to min/max). Crest Isle: `[` / `-` closer, `]` / `=` farther, mouse wheel on `Walk.update`. HUD + console + docstring. Engine `get_key_code` / `character_to_keycode` map those glyphs.

## Left in place

Blob AO, tile UV period/blend/pad, sparks, hair rim, gold coins, Mixamo `bind_locomotion`, spatial listener, sticky-walk quiet gap 3, opaque title, Mesh3D cap 256, chase clamp constants (`CAM_MIN_DISTANCE=6`, max = authored hypot). No SSAO / CSM / volumetric fog / Rapier / editor / WebXR.
