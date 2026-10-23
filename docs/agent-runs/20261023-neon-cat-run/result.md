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

Title card: a touch on 「はじめる」 calls `startGame`. The sim is title
under that card (no CRASH column, cat back at the start). Chrome on
lavapipe, mobile viewport, is the check for the tap.

Deaths: the side plane is a clamp, not a kill. Crates and barrels load
(UNSIGNED_BYTE indices). The kill box is inside the mesh. The result
names the prop (`HIT CRATE`). `?debug=1` draws the mesh cage.

The `/pr/165/` wasm Emma played was still the build before that commit
(`unsupported index componentType`, result word `CRASH`, no `setDebug`).
A path ending in `crate.glb` was also drawn as the embedded unit cube.
A bare `crate.glb` stays that alias. A real path loads the file. A mesh
that fails to load is not spawned. Portrait `412×915` on the 8-bit
path shows the Kenney crate at `quality=mobile` (`ssao=4 c2 post`) and
`quality=low` (`ssao=0 c1 post`).

The default camera is a damped third person: 5.9 m behind, 2.35 m up,
looking 13 m down the tunnel, so the cat is about 15% of a 412×915
frame and sits in the lower third. FOV opens from 50° to 58° with
speed. Steering banks the rig by at most ~0.07 rad. Landing uses
`Juice.shake(0.08, 0.18)`. `?cam=classic` keeps the previous chase.

The picture is a dark navy tunnel. Exposure 0.70, bloom 0.16 at
threshold 1.4, fog from 38 m. The floor checker is near-black. Grid,
rails, and wall wireframe are fat lines. The arch is the dark
Quaternius mesh plus a neon outline. Hoops are cyan. Obstacles are a
dark body with a cyan or magenta edge cage. The cat tint is a dark
violet at roughness 0.9. Portrait 412×915 on the 8-bit path is the
check for `quality=mobile` and `quality=high`.
