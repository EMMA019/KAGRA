# 2026-10-22 — glTF biplane, pale chess highlights, backyard match flow

## User request

The box-built biplane silhouette was broken (beige wing plates). Prefer a real airplane `.glb`. Chess background still read as flat navy; legal-square mint was too strong. Backyard look is shippable, so add title / time limit / win-lose. Also answer what games the engine can make now and what to do next.

## Decisions

1. **Hero mesh is one glTF, not stacked primitives.** `assets/models/biplane.glb` (original CC0 mesh, 6 materials: tapered fuselage, cambered wings, N-struts, gear, canopy, spinner). Aliases `biplane` / `airplane` / `plane_glb` in `kagra-shared/src/assets.rs` and `kagra/contracts.py`. The repo has Quaternius nature / medieval kits and KayKit cars, and no airplane pack, so this mesh was built in-repo rather than downloaded. The spinning propeller stays a parented box on `prop:plane`. Instance color is `[255,255,255]` so the atlas is not tinted.
2. **Studio floor gradient is a shader path that only chess turns on.** `shader3d.wgsl` darkens solids when `world.y < -0.12` and IBL `< 0.30`. Chess dump sets `ibl` 0.28 and a lighter floor `[54,68,92]`. Flight and backyard stay above that IBL so the island and yard are unchanged. Bloom vignette floor dropped from 0.54 to 0.28 with a wider ellipse.
3. **Chess highlights stay on the marble.** Cursor / selected / legal squares are desaturated tints of the light and dark squares. Legal moves also get a small `marker` cylinder so the tile count stays 64 and contact blobs skip markers.
4. **Backyard match flow stays in Python.** `phase` is `title | play | result`. `MATCH_SECONDS` is 90. Three captures or the clock writes a result banner. Confirm from the result returns to the title. HUD strings use ASCII hyphens because Public Sans covers Latin up to U+024F and an em dash was drawing as a missing glyph.
5. **No new WorldPlay genre.** Chess, backyard, and flight remain free `WorldDoc`s.

## Verify

- `cargo fmt -p kagra-shared -- --check` clean
- `python3 -m pytest tests/test_backyard_brawl.py tests/test_flight.py` passed
- `python3 tools/gen_api_index.py --check` OK (56 entries)
- Same tree, before the ASCII-hyphen tweak: `cargo test -p kagra-shared --lib --locked` 398 passed, `pytest tests -m "not golden"` 646 passed, chess / backyard / flight smokes `ok: true`
- Offscreen: `chess_studio.png`, `flight_clear.png`, `flight_bank.png`, `backyard_title.png`, `backyard_victory.png`, `backyard_brawl.png`
