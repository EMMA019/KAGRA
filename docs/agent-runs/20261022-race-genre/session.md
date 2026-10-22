# Race GenreSession

## Prompt

Expose the race stack (`race.rs`, `vehicle.rs`, `driving.rs`, `road.rs`) as a `GenreSession`, with a 3-lap example, lap timer, best-lap ghost, smoke, and ASCII HUD. Shared edits stay additive.

## Decisions

- Class name `RaceSession`. Genre id stays `race_drive` (`race::GAME_ID`). The session wraps `WorldPlay`. The existing 1-lap box track is unchanged: no `waypoint` props, `laps_need` stays `LAP_NEED` (1).
- A dump with 4+ `waypoint` props drives `vehicle::Truck` on `road::RoadPath`, kept on the ribbon with `collide::apply_road_bounds` (the same bounds `driving.rs` uses). Top speed comes from `driving::practical_top_speed`. `lx` is steer (right positive), `lz` is throttle / brake.
- Lap count is the disabled prop `laps` scale.x (3 on the new course). The lap clock is `RaceGame.lap_time` / `best_lap`, mirrored onto the dump timer named `lap` (`active: false`, so it is not a countdown).
- Crossing finish while armed commits the lap. A faster lap replaces `best_line`. The `ghost` prop (KayKit taxi) replays that line. The player car is the KayKit sedan. Roads are KayKit `road_straight` / `road_corner`. Trees, pine, and rocks use the existing aliases.
- On-screen L / R / ACCEL pads are HUD quads. Text uses ASCII hyphen only (`race - lap 1/3`, `best --`).

## Verify

- `cargo test -p kagra-shared --lib --locked` — 404 passed
- `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` — clean
- `cargo clippy -p kagra-shared --all-targets --locked --no-default-features --features render -- -D warnings` — clean
- `cargo clippy -p kagra-shared --all-targets --locked --features python -- -D warnings` — clean
- `cargo fmt -p kagra-shared -- --check` — clean
- `pytest tests -m "not golden"` — passed (venv with `maturin develop --release`)
- `python examples/race_minimal.py --headless` — won, 3/3 laps, best 4.98s, ghost on (lavapipe)
- `python -m kagra.verify examples/verify_scenarios/race_smoke.json` — ok, offscreen not skipped
- Other current smokes ok on lavapipe except three that already fail without this change: `demo_live_smoke` (script missing), `physics_stack_smoke` (png only when the physics feature is built), `rpg_town_smoke` (fixture walker name is `hero`, smoke asks for `player`)
