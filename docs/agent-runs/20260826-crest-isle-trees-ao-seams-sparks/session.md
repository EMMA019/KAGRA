# Session — Crest Isle black trees / blob / seams / sparks

## Confirmed (no Blender)

Opened `forest/tree-high.glb` / `forest/tree.glb`:

- `images[0].uri` = `Textures/colormap.png` (external, not bufferView).
- PNG is on disk at `examples/assets/open_world/kenney/forest/Textures/colormap.png` (10,659 bytes).
- Material `colormap`, `metallicFactor: 0`, `KHR_texture_transform: {texCoord:0}` (offset/scale default to identity).
- Nature Kit pines/oaks have **no** `images` and use `KHR_materials_unlit` + vertex `baseColorFactor`.

Prop path is Python `flatten_gltf` → `Prop.bake`, not `kagra.load_gltf`. Engine Mesh3D fallback is actually **white** 1×1 (`Fallback White`), not black; missing atlas still reads as untextured silhouettes / wrong atlas cell.

`_read_relative_image` already existed on master. This pass:

1. Treats GLB-relative URI resolve as the required contract (tests on the real Kenney files + a synthetic `uri: Textures/colormap.png` GLB).
2. Applies `KHR_texture_transform` in `flatten_gltf` and in Rust `load_gltf` (identity `{texCoord:0}` stays a no-op; offset/scale atlas UVs now land).

## Terrain knife

`heightfield_mesh` used **in-tile** one-sided height diffs, so adjacent `stream_tiles` disagreed on the shared edge normal → straight lighting join. Sampler is `ClampToEdge`, so UVs outside 0..1 sample the JPEG dirt border (banding). Shared code now samples `fn` *outside* the tile (Relic/Overworld keep default UV). Crest Isle only sets `terrain_uv_period=13.5` (≠ TILE 16), `uv_blend=2.6`, `uv_pad=0.035`.

## Fake AO / particles

No SSAO. Blob is `quad_y_mesh` + alpha ellipse (`draw_mesh_3d(..., skip_fog=True)`), always-on in play (title still skips the live island). Sparks are CPU `SparkBurst` in `open_world_rules.py` (no kagra import), drawn with existing `draw_billboard_instances`.

## Stumbles

- Snapshot already had relative-URI helpers + Kenney tests; the hole was KHR UV + no real `tree-high.glb` flatten assertion, plus tile-edge normals.
- Camera-facing `draw_billboard` would stand the blob up; used Y-quad equivalent as allowed.
- Did not invent a particle draw API.

## Leftover polish (same PR)

Hair: `apply_outdoor_look` does **not** call `set_rim` (global rim would light face + hair and risk the white mask). Per-material: `is_hair_material` matches Hair / 髪 / bangs and skips face/skin/eyes. `boost_hair_rim` writes a warm rim (not 1,1,1) and lift 0.62 after VRM0 overlay so authored `_RimLift: 0` cannot wipe it. Shader keeps `if !front { n = -n; }`. Unnamed hair with lift ~0 gets a dark/saturated albedo silhouette only; pale skin stays off.

Gold: Kenney `dungeon/coin.glb` is a painted yellow disc — metallic 0.85 / roughness 0.22 still reads plastic. Crest coins are now `Prop("sphere", color="gold", metallic=1.0, roughness=0.12)` (pretty-room chrome path). Kenney crests (flags/banners/chests) stay. Relic Run orbs not retuned.

GitHub CI: `1ad693e` 17 green, then head `20fd713` 17 green.
