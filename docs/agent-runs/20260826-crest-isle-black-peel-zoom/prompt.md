Emma just merged PR #91 and playtested Crest Isle. She says it is much better, BUT:
1. Some trees are still solid black silhouettes (mint-green Nature Kit pines look OK; at least two black trees remain mid-ground left and further back).
2. Moving the camera makes things "bald/peel" (はげる): textures/geometry drop or strip. Her shot shows: stretched smeared grass in the foreground, a thick rectangular white fog slab on the right (not a natural horizon), dark gray/black rectangular ground patches like missing tiles/textures, distant trees popping flat. Could also be VRM hair going bald when the camera orbits (prior MToon inside-skull / bind-group eviction). Treat both: world textures peeling AND hair stripping.
3. She wants commands to zoom the chase camera closer and farther (keyboard is fine: `[` `]` or `-` `=` / mouse wheel). Existing chase cam already has min_distance/max_distance clamp — expose player control, keep the clamp so it cannot explode into the skull or fly to a speck.

Her screenshot (describe, no file): window title "VRM Crest Isle"; small pink-haired VRoid in white outfit with a blob shadow under feet; flat grass with a large sandy rectangle; mint low-poly pines OK; 2+ solid-black tree silhouettes; right half washed in a rectangular white fog overlay; foreground grass smeared/stretched; distant ground has dark missing-texture rectangles.

Repo: https://github.com/EMMA019/KAGRA. Demo: `python examples/vrm_open_world.py`. Start from current master (PR #91 is merged).

Black-tree context: #91 made flatten_gltf resolve GLB-relative `Textures/colormap.png` and KHR_texture_transform. Forest tree-high.glb/tree.glb use that external atlas (PNG exists next to the glb). Nature pines are unlit vertex color (mint). If some trees are still black, likely (a) runtime resolve uses cwd not the glb dir, (b) Mesh3D LRU still evicts live Kenney colormap when the camera reveals more props, (c) a different placed GLB still broken, (d) KHR_materials_unlit + metallic=1 lighting vertex-color trees black. Do not stop at "#91 should have fixed it". Confirm which files in examples/open_world_rules.py / chunk props stay black.

Peel context: Mesh3D LRU max 256 never-evict-live-diffuse can still fail if live is wrong when the camera moves; stream_tiles may unload/reload a ring as missing; fog may draw as a screen-aligned slab. Investigate; do not guess only one cause.

Constraints: no SSAO, no 4-cascade CSM, no volumetric fog, no Rapier, no Unity editor, no Web/XR. Keep blob AO, tile blend, sparks, hair rim, gold coins, Mixamo bind_locomotion, spatial listener, sticky-walk quiet gap 3, opaque title. Do not merge; Emma merges.

Ship: remaining black trees gone (or swapped to a Kenney that actually colors), camera orbit/pan no longer strips grass/trees/hair or paints a white rectangle, zoom in/out keys (document in PR + one-line HUD or console). Tests for zoom clamp + texture URI resolved from glb dir not cwd. Open a PR.
