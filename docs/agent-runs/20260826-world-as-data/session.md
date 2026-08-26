# Session

- Read `kagra/world3d.py`, `kagra/__init__.py`, `kagra.verify`, Orb Rush / Crest Isle, verify scenarios. Did not touch `stream_tiles` / `_upload_tile` / `uv_rect` / `TERRAIN_UV_*` (PR #97). Relic Run UV defaults unchanged.
- `kagra/world.py`: `World = World3D`. `query` / `dump` / `load` bound onto the class. String ids `prop:N`, `walker:player`, `light:slot`, `camera:main`, `tile:ix,iz`.
- Bald leftover = key in `_loaded_tiles` AND `_terrain_tex > 0` AND (no mesh or 1×1/missing albedo). GPU-free stream (`_terrain_tex == 0`) is not bald.
- Lights recorded in `kagra.look` from `set_point_light` / `set_spot_light`. Walk registers on `world._walkers`.
- Public table: dropped Entity / tilemap / Tk from `import kagra`. Archive 2D now `from kagra.entity import World, EntityScene`.
- Crest Isle constructs `kagra.World`, names coins `coin` / crests `crest`, dumps JSON on smoke quit. Orb Rush constructs the same `World`, queryable star/bomb Props (drawing stays billboards).
- `kagra.verify` `expect_world` (player.on_ground, coins, query counts). PNG file-size remains smoke only.
- Roadmap rewritten. Old 63% archived under `docs/archive/`. AGENTS.md / README / AGENT.md / skill / recipes / REVIEW pointed at the new definition.
- Stumbled: `tools/gen_api_index.py` `main()` had been nested after a `return` in `_runtime_enrich` during the first pass; restored. AST `__all__` is runtime-computed so `_PUBLIC_OFF` is the deny list for the index.
