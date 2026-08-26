Emma said keep shipping until she checks ~21:00 JST. No extra chatter. Open ONE PR against master with this sequence. Do not merge. Do not add SSAO, 4-cascade CSM, volumetric fog, Rapier, a visual editor, Web/XR, or Mixamo binaries.

Repo: https://github.com/EMMA019/KAGRA. Demo: `python examples/vrm_open_world.py` (Crest Isle). Recent master already has sleeve cloth, locomotion blend, spatial audio, multi-avatar GPU share, Mixamo VRoid rest/roll compensation (#90).

## Already diagnosed (do not redo Blender)
Kenney `examples/assets/open_world/kenney/forest/tree-high.glb` and `forest/tree.glb` are NOT missing files and are NOT vertex-color unlit:
- glTF image is an **external URI** `Textures/colormap.png` (not bufferView / not embedded). The PNG **exists** at `examples/assets/open_world/kenney/forest/Textures/colormap.png` (10,659 bytes).
- Material `colormap`, doubleSided, baseColorTexture index 0, metallicFactor 0, `KHR_texture_transform` present (currently `{texCoord:0}` in the JSON), NORMAL+TANGENT+TEXCOORD_0.
- Contrast: `nature/tree_pineTallA.glb` / `tree_oak.glb` have **no images**, `KHR_materials_unlit`, vertex `baseColorFactor` only. Those should already look colored. Forest trees go black if Prop/glTF loading does not resolve the GLB's directory-relative image URI (fallback black 1x1) and/or ignores KHR_texture_transform atlas UVs.
Crest Isle places `forest/tree-high.glb` via `examples/open_world_rules.py`.

## Ship in this order (all in this PR)
1. **Black trees.** Make GLB-relative image URIs resolve next to the `.glb`. Apply KHR_texture_transform if the loader ignores it. Forest Kenney trees must show the colormap, not black. Add a GPU-free test that a glb with `uri: Textures/colormap.png` finds that PNG. Do not drop the tree assets; do not require Blender re-export.
2. **Fake AO.** Character blob shadow: a soft translucent ellipse under the VRM feet via existing `draw_billboard` (or equivalent). Cheap, always-on in Crest Isle. No SSAO.
3. **Terrain seams.** `stream_tiles` / heightfield chunk borders currently hard-edge (grass/dirt pop). Blend across tile boundaries enough that the seam is not a straight knife line. Also reduce obvious albedo banding if it's the same path. Crest Isle meadow only; don't retune unrelated demos unless shared terrain code requires it.
4. **Lightweight particles.** No physics particle engine. A small CPU list of {position, velocity, life, fade} drawn with `draw_billboard` / `draw_billboard_instances` if that API exists (search `docs/API_INDEX.md` first; do not invent APIs). Crest Isle should spawn a little pickup/magic-ish burst so the path is visible. Tests for spawn/expire/fade without GPU.

Constraints: no Unity/Tk editor; agent eyes stay `kagra.annotate` / `kagra.debug_trace`. Keep sticky-walk quiet gap 3, Mesh3D LRU 256, chase cam clamp, opaque title, spatial listener, set_locomotion, Mixamo bind_locomotion. Tests pass (`pytest tests -m "not golden"`, rust tests you can run, API index check). GPU smoke optional if no wheel.

Investigate, implement, test, open the PR. Report in the PR body: black-tree root cause you confirmed, what shipped, how to try, what you left out.
