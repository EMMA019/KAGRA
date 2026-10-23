# Result

Neon Cat is a selectable game at `play.html?game=neon_cat`. Landing stays
`?game=neon_run`.

- Sim: `kagra-shared/src/neon_cat.rs` (`SceneKind::NeonCat`)
- Host: `examples/neon_cat_minimal.py`
- Manifest: `games/neon_cat_minimal.json`
- Smoke: `examples/verify_scenarios/neon_cat_smoke.json`
- Assets: `tools/fetch_assets.py`, `docs/assets.md`
- Shots (lavapipe): `docs/screenshots/neon_cat_title.png`,
  `neon_cat_play.png`, `neon_cat_result.png`, `neon_cat_portrait.png`

`cargo test` neon_cat (5) and the rigid-clip test passed. Clippy `-D warnings`
passed with and without `render`. `package_web --skip-wasm` staged the cat,
arch, crate, barrel, hoop, and barrier GLBs. Web preview tests still put
Lumen first.

The attached reference image was not in the checkout when the shots were
taken, so the PR shows the lavapipe frames on their own.
