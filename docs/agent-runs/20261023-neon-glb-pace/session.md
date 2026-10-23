# 2026-10-23 — Lumen GLBs and a slower opening

## What the build was

`NeonRun::rebuild` drew every mesh as a primitive:

| thing | before |
|---|---|
| player | sphere `ship-core` plus box nose and wings |
| obstacles | torus rings (`model: ring`); a miss outside `HOLE_R` 1.15 crashes |
| pickups | gold sphere `orb` |
| environment | box ribs, rails, and islands; sphere stars |

`games/neon_run_minimal.json` bundled only `assets/audio`. `scripts/package_web.py` copied wav/ogg/mp3 and nothing else. `kagra-shared/www/play.html` fetched `world.json` and audio. It never called `mountFile`. The wasm runtime can only see a GLB after that mount (there is no game directory on wasm). So the live page loaded no `.glb`.

## Decisions

1. Vendor seven Kenney Space Kit 2.0 GLBs (CC0) under `assets/models/kenney/space-kit/`, with `LICENSE.txt` naming the official zip. The pack's root translation `[2, 0, 1.5]` was zeroed so each mesh sits on the origin. Vertices and materials are otherwise the file Kenney shipped. No external textures.
2. Attach a GLB only when `mesh_from_glb` succeeds. The prop keeps its primitive `model` (`sphere` or `box`). A missing or unmounted file leaves `gltf` unset, so the primitive draws. The page mounts `assets.json` before `loadGameJson`, because the first frame bakes the mesh and a late mount would stick on the fallback.
3. Ship is `craft_speederC.glb` (yaw π so the heavier tail faces the chase camera). Obstacles around each ring are meteor / rock / turret GLBs, placed outside the pass radius. The ring torus stays, scaled to the hole, so the target still reads as neon. Pickups are `rock_crystals.glb`. Side islands are `satelliteDish.glb` plus the emissive rim. Ribs, rails, and stars stay procedural.
4. Pace: cruise 14.3 (was 22, about 35% slower), ramp 0.32/s (was 0.62) up to the same cap 46, boost still +9. First gate at z=50 (was 34). Early gap 44 tapering to 23 (was a flat 26), so the late rings come a little faster than before. Grace 2.4 s: softer steer, a shrinking hold inside the corridor, and a miss does not crash. Walls 2.58 / 1.90 (were 2.35 / 1.72). Hole 1.46 tapering to 1.28 (was 1.15). Early holes stay near the middle.
