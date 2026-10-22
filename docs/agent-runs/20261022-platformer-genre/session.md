# Platformer GenreSession

## Prompt

Expose `kagra-shared/src/platformer.rs` (`box_hop`) as a `GenreSession`, with a side-scrolling example, smoke, and ASCII HUD. Shared edits stay additive.

## Decisions

- Class name `PlatformerSession`. Genre id stays `box_hop` (`platformer::GAME_ID`). The session wraps `WorldPlay`; jump / land / fall / checkpoint / goal stay in Rust.
- New dump `platformer_world.json` (does not replace `box_hop_world.json`). Slabs are `crate.glb` so the unit-cube mesh matches the AABB the platformer already uses. Knight, chest, well, rock, and stylized trees are bundled aliases.
- Camera sits on +Z so `look_yaw` is pi and +lx (D / on-screen R) runs toward +X. Three gaps, scripted hops in the Rust test and in `--headless`.
- On-screen L / R / JUMP pads are HUD quads. Text uses ASCII hyphen only.

## Verify

- `cargo test -p kagra-shared --lib --locked` — 402 passed
- `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` — clean
- `cargo clippy -p kagra-shared --all-targets --locked --no-default-features --features render -- -D warnings` — clean
- `cargo fmt -p kagra-shared -- --check` — clean
- `pytest tests -m "not golden"` — passed
- `python -m kagra.verify examples/verify_scenarios/platformer_smoke.json` — ok on lavapipe (pixels not_solid, not skipped)
- Other current smokes ok on lavapipe except three that already fail without this change: `demo_live_smoke` (script missing), `physics_stack_smoke` (png only when the physics feature is built), `rpg_town_smoke` (fixture walker name is `hero`, smoke asks for `player`)
