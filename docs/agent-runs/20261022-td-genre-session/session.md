# Tower defense GenreSession

## Prompt

Expose `kagra-shared/src/td.rs` (`td_lane`) as a `GenreSession`, with a small map, five waves, two or three tower types, gold, lives, win/lose and retry, tap and mouse placement, bundled glTF, and an ASCII HUD.

## Decisions

- Class name `TdSession`. Genre id stays `td_lane`. The session wraps `WorldPlay`. Spawn, damage, gold, lives, and placement stay in `td.rs`.
- The existing lane fixture is unchanged: one wave, one box tower, cost 5, clear only when nothing leaks. Creep ids shaped like `prop:creep-w3-1` turn on wave mode.
- Wave mode: 5 waves (19 raccoon / wolf creeps), 15 lives, 80 gold. A leak spends a life and ends the run at 0. Clearing the last wave with lives left wins. `confirm` on the result screen reloads the seed (retry).
- Three towers, placed by tap or mouse on a slot, or by standing on a slot and pressing J: ARROW (`gazebo`, 8), CANNON (`mill`, 18), MAGE (bell `tower`, 28). Each has its own range, damage, and cooldown. Kills pay 2 gold. Buttons and the status line are ASCII (`td_lane - wave 1/5  gold 80  lives 15`).
- New dump `td_world.json` (does not replace `td_lane_world.json`). Path slabs and pads are `crate.glb`. Knight, chest, well, rock, trees, and pines are bundled aliases.
- Keys 1/2/3 select a type. The keymap addition is three entries.

## Verify

- `cargo test -p kagra-shared --lib --locked` — 426 passed (after merging race and puzzle from master)
- clippy `-D warnings` default, `render`, and `python` — clean
- `cargo fmt -p kagra-shared -- --check` — clean
- `pytest tests -m "not golden"` — passed
- Headless lavapipe: `td_smoke` ok (pixels not_solid). `examples/td_minimal.py --headless` cleared all 5 waves (`won=True lives=15 kills=19 leaks=0`). Screenshot `scratch/td_minimal.png` shows the lane, three towers, creeps, and the ASCII HUD.
- Other current smokes ok on lavapipe except three that already fail without this change: `demo_live_smoke` (script missing), `physics_stack_smoke` (png only when the physics feature is built), `rpg_town_smoke` (fixture walker name is `hero`, smoke asks for `player`)
