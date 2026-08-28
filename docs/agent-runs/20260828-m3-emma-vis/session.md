# Session

Worked on Emma Windows PC D:\program\kagra at origin/master 4c9c00d (LOD + GPU instancing). Did not clone.

Official play dump `emma_walker_world.json` is still heightfield=null, props=[], water_y=null, coins=0, player.model=capsule + gltf=assets/Emma.vrm.

Why the void + capsule:
- compile_scene only pushed MESH_HEIGHTFIELD when heightfield was Some. Clear color [130,165,205] plus a PCF contact blob = blue void + diamond.
- Window title else-branch was always "KAGRA Crest Isle". Playing HUD always drew STAR_XZ (8) gray pips even when not a collectathon.
- `model: capsule` is dump style; walker_gltf_spec already prefers `gltf`. A stale `target/debug/examples/window.exe` (play_world prefers the prebuilt helper) still drew the teal/tan capsule+head path. Relative `assets/Emma.vrm` from that cwd also misses repo-root unless CARGO_MANIFEST_DIR / KAGRA_ROOT / exe-walk resolve it.
- If Emma.vrm was on disk but parse failed, load_skinned used to `return None` and compile_meshes swapped in a box — still not a VRoid. Now parse/missing is dump-visible `load_error` and the walker slot stays tpose_humanoid (8-vert skinned), never MESH_CAPSULE.

Changes:
- assets: repo_root (KAGRA_ROOT, CARGO_MANIFEST_DIR/.., cwd/exe ancestors) + resolve_asset (alias emma → assets/Emma.vrm).
- play_world.py sets KAGRA_ROOT on the window subprocess.
- world_doc: floor plane (Grass MESH_PLANE at floor_y) when heightfield is none. Walker gltf slot never capsule; load fail → tpose_humanoid. refresh_asset_status writes dump-visible load_error.
- world_play: refresh_asset_status in new(); star pips only for collectathon; coin pips only when coins>0.
- window: desktop_title from dump stem (emma_walker_world) unless a genre/collectathon; prints walker load_error.

Kept Mixamo/MToon/albedo/spring/morph/look-at. No Bevy / RendererV2 / Relic Run UV change. as_chunks unchanged.
