# Session — Crest Isle mobile slice

## API / architecture

- Searched `docs/API_INDEX.md` and the collectathon branch: desktop game is `examples/vrm_open_world.py` + `open_world_rules.py` + `kagra.land.open_world_height`.
- `kagra-shared` is a separate wgpu-30 renderer (C ABI + wasm). `SceneKind` was Driving / Demo2D only.
- Kenney GLBs live under `examples/assets/open_world/` and are **not** in the pip wheel (`pyproject.toml` include is `kagra/data/*` only). Mobile does not ingest those GLBs (gltf_load is JSON-only; dumping GLB into the shared crate would bloat wasm / still not VRM).

## Decisions

- New `SceneKind::Collectathon` (`set_scene(2)`). Default session stays Driving so existing haul tests keep working.
- Port `open_world_height`, 8 crest XZ, coin path, score/grade, jump (7.2) into `kagra-shared/src/collectathon.rs` (GPU-free).
- Player is a **teal capsule + hood** (Kenney-style stand-in). **Not VRM. Not Alicia.**
- Terrain is a heightfield mesh + water plane + cone trees / box rocks / flag poles / gold discs. Same opening vista idea (grass, west sea cliffs, north pines).
- Touch: left virtual stick, right jump. Also `set_walk(lx, lz, jump)`.
- Wasm page: `kagra-shared/www/crest.html`. Android / iOS shells boot scene 2.
- Corridor Haul remains at `www/index.html` and `set_scene(0)`.

## Stumbles

- Cylinder cap indices: first draft pointed both caps at the top ring. Fixed (`top_c + 3` for the bottom rim).
- `set_scene(2)` must `boot_collectathon()` so Android `create()` + `setScene(2)` lands on the title, not a half-initialised walker.
- Pointer stick must clear on End, or a lift would keep walking.

## Verify

See `result.md`.
