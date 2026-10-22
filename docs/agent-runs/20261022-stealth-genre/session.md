# Session

Read `AGENTS.md` (how to add a genre), `genre_session.rs`, and the platformer / novel / puzzle / race / td / rhythm / action / fight bindings. Local `master` was behind `origin/master` (`3b5f9f3` vs `55ab2ec`, the RPG session merge). Branched `cursor/stealth-genre-session-ea16` from `origin/master` so `RpgSession` and `ShopSession` were already present.

`StealthSession` is a thin `WorldPlay` wrapper for genre `stealth_hide`. Patrols, vision cones, noise, and the alert meter stay in `stealth.rs`. A dump with no `patrol` props keeps the old one-tick hide / cone / exit behavior (`CONE_RANGE` 6, `CONE_HALF` 0.68). A yard with `patrol` props ping-pongs guards, ramps alert (see / hear / decay), hears noise (crouch is quieter), and blocks line of sight on `wall` / `hide` / `block`. The exit clears only after the `key` prop is picked up when one exists. Confirm reloads the seed, so a catch retries with the key enabled again.

Crouch is `WalkInput.dodge`. `WorldPlay` walks a slowed stick (`CROUCH_SCALE` 0.55) and passes the original stick into `stealth::tick` so noise still sees a full step, then clears dodge. Cones are props named `cone` parented to a guard. `mesh_for_prop("cone")` now maps to `MESH_CONE`. `place_room_camera` no longer overwrites an authored camera; each tick restores that eye so the chase camera does not win. `sit_named` clones the walker so the KayKit glTF survives a height snap.

The scripted route waits by the dagger until the hallway guard is walking +X and alert is low, then crouches through the door gap and commits to the exit. The first version stopped in the doorway: the next waypoint sat on the same side of the divider as the player, so the wish went to zero. Thresholds now step through `(0, -1.25)` once `z` is past the door mouth.

Clippy `-D warnings` rejected `Vec<(String, [f32; 3], f32, f32, [f32; 3])>` in `sync_cones`. A small `ConePose` struct replaced the tuple. `segment_hits_rect` takes one `[f32; 4]` so the slab test stays under the argument-count lint.

Shared edits stayed additive: `py/mod.rs`, `lib.rs` re-export, `kagra/__init__.py`, `FRONT_NAMES`, `tests/test_play_world.py`, one `mesh_for_prop` arm, the stealth walk scale in `world_play.rs`, and `c` / Control in the gameloop keymap.
