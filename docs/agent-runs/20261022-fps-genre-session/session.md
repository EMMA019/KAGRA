# FPS GenreSession

## Prompt

Expose the existing `fps_range` module as a `GenreSession`, with a three-wave arena, ASCII HUD, KayKit assets, mouse/WASD plus touch, and `fps_smoke.json`. Shared edits stay additive. Merge latest master before the PR.

## Decisions

- Class name `FpsSession`. Genre id stays `fps_range` (`fps::GAME_ID`). The session wraps `WorldPlay`. The existing range fixture (static props named `target`) is unchanged: no `enemy` or `reserve` walkers, so win is still "every target disabled".
- Arena combat lives in `fps.rs`. Walkers named `enemy` chase and contact-damage. Walkers named `reserve` are later waves, buried at y=-40 by `fps::seed` until the live wave is dead, then renamed `enemy` and stood up. Death renames the walker `down` and buries it. Win when no enabled shootable prop and no `enemy`/`reserve` walkers remain. Lose when HP hits 0 (`PLAYER_HP` = 5). `confirm()` calls `start()`, which restores the seeded dump.
- Hitscan on an enemy uses the body center (`ground + BODY_H`, radius `ENEMY_R`), so a feet-origin mesh still takes a torso shot. Contact is XZ only. KayKit rest-pose min Y is 0, so `stand_y` plants the feet on the floor. The player capsule is pinned every FPS tick to `ground + BODY_H` and `vy` is forced to 0 so the eye stays at head height. Jump is unused by the example.
- Ammo stays the existing mag (`coins` = 6). Attack fires, dodge reloads. `kagra.gameloop` maps the R key so the example can reload from the keyboard.
- Python owns the pads and the HUD. Left stick moves, a right-half drag looks, FIRE and RELOAD are buttons. A tap that is not a look-drag fires. Headless play aims with `atan2`, kites when the nearest enemy is close, and reloads on an empty mag. HUD strings use an ASCII hyphen (`clear - retry`, `down - retry`).
- Props and walkers use the existing aliases: `knight`, `rogue`, `mage`, `barbarian`, `crate.glb`, `stylized_tree`, `stylized_pine`, `rock`, `well`. Characters are skinned walkers (rest pose). The scenic verify camera is not named `eye`, so the knight stays visible in the offscreen shot. First-person play hides the local body.

## Stumbles

- The first combat frame floated the KayKit bodies because seed placed them at the capsule center (`height + BODY_H`) while the rest-pose feet sit at local Y 0. `stand_y` plants the feet. Hitscan still aims at the torso.
- A player gltf at feet height dropped the eye to the ankles. `pin_player_capsule` plus `vy = 0` keeps the eye on the head. Range tests still pass.
- A bulk verify loop parsed a nested JSON object (stdout is pretty JSON plus a `RuntimeWarning`) and reported every smoke as failed. `JSONDecoder.raw_decode` shows the real result: `fps_smoke` and `fps_range_smoke` are ok. `rpg_town_smoke` fails on master too (walker name is `hero`, the smoke asks for `player`). `demo_live` and `physics_stack` stay the known skips.
