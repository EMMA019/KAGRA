# Session

## What was already there

`survival.rs` is the `survival_meter` WorldPlay slice: a hunger meter that drains, fills in the camp box, and ends on `ok` or `starve`. Those tests stay. `WorldPlay` was not given a new genre branch.

On this checkout the GenreSession bindings were platformer, novel, puzzle, and race. The task named td, rhythm, action, fight, rpg, and shop as well; those land on master from other branches and are merged at the end.

## Decisions

- `SurvivalSession` (`genre: "survival"`) lives in `survival.rs` and implements `GenreSession` in `genre_session.rs`. Python is `PySurvivalSession`.
- Rust owns the clock (22s day, night after 0.62), hunger, health, wood, food, campfire (4 wood), and gazebo shelter (4 wood). Night damage applies outside the fire or shelter radius. Empty hunger drains health. Dawn of day 4 with both structures is a win.
- The clearing uses bundled meshes: knight, stylized_tree, stylized_pine, rock, well, gazebo, Bonfire.glb, Mushroom.glb, plus flower / grass / fern. The grass floor is a box. Campfire and shelter start disabled and turn on when crafted.
- `demo_input` is the headless autopilot so the unit test and `survival_minimal.py --headless` share one path. Keyboard and the on-screen stick / GATHER / EAT / BUILD stay in Python. HUD copy is ASCII.
- `expect_world` counts `walkers[]`, not the `player` object. The first smoke failed with `walker name=player got 0` until the fixture mirrored the player into `walkers`.

## Verify

- `cargo test -p kagra-shared --lib --locked` — 425 passed.
- `cargo clippy -p kagra-shared --lib --locked -- -D warnings` and the same with `--features render` and `--features python` — clean.
- `pytest tests -m "not golden"` — passed (a few skips where the extension or optional assets are absent in some tests).
- `python -m kagra.verify` on `examples/verify_scenarios/*.json` with lavapipe — `survival_smoke` ok. `demo_live_smoke` (missing script) and `physics_stack_smoke` (png not written) still fail. `rpg_town_smoke` still fails (fixture walker is `hero`, query asks for `player`); not touched here.
- Headless lavapipe: `examples/survival_minimal.py --headless scratch/survival.png` — won, day 4, fire and shelter, status `survived 3 days`.
