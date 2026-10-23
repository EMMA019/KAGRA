# Result

## GLB findings (before this change)

Lumen did not load any `.glb` or `.gltf`.

- Player: sphere `ship-core`, box nose, two box wings.
- Obstacles: torus rings (`model: "ring"`). Missing the hole crashed the run.
- Pickups: gold sphere `orb`.
- Environment: box ribs, rails, and islands, plus sphere stars.
- Web/wasm: `games/neon_run_minimal.json` bundled only `assets/audio`. `package_web.py` copied audio. `play.html` never called `mountFile`. Wasm has no game directory, so a path in the dump would not have loaded even if one had been set.

## What shipped

Kenney Space Kit 2.0, CC0, from the official zip. License: `assets/models/kenney/space-kit/LICENSE.txt`. The pack's root translation `[2, 0, 1.5]` was zeroed so each mesh sits on the origin.

| role | file | fallback |
|---|---|---|
| player | `craft_speederC.glb` | sphere + nose + wings |
| gate obstacles | `meteor.glb`, `meteor_detailed.glb`, `rock_largeA.glb`, `turret_single.glb` | sphere |
| pickup | `rock_crystals.glb` | gold sphere |
| side scenery | `satelliteDish.glb` | the old island box |

Ribs, rails, the neon ring, and stars stay procedural. `play.html` fetches `assets.json` and mounts those bytes before `loadGameJson`. A failed fetch leaves the primitive.

Pace: cruise 14.3 (was 22, about 35% slower), ramp 0.32/s (was 0.62), cap still 46, boost still +9. First gate at z=50. Early gap 44, late gap 23. Grace 2.4 s (`EASY START`). Walls 2.58 / 1.90. Hole 1.46 tapering to 1.28.

## Verify

- `cargo test -p kagra-shared --lib --locked neon_run` — 3 passed (GLB attached, opening speed, grace, wall still kills).
- `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` — clean.
- `cargo fmt -p kagra-shared -- --check` — clean.
- `pytest tests/test_neon_run.py tests/test_browser_preview.py` — passed. The staged web tree contains the seven GLBs and `play.html` calls `mountFile`.
- Phone-size Chromium (844×390 and 390×844, touch, mobile UA) against the packaged wasm page. WebGPU had no adapter; the page fell back to WebGL2/SwiftShader and drew the speeder, the grace banner, and Kenney rocks around a ring.
