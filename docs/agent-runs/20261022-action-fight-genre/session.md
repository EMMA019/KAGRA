# Session

Read `AGENTS.md` (how to add a genre), `genre_session.rs`, and the platformer / novel / puzzle / race bindings on origin/master (`3b5f9f3`, after the local snapshot). Two sessions, same thin `WorldPlay` wrapper:

- `ActionSession` (`action_arena`) — hitboxes, HP, dodge i-frames, chase AI, a 2-hit combo window, and the boss wave.
- `FightSession` (`fight_hitstun`) — existing guard / hitstun / combo / opponent AI, unchanged rules.

Arena fixture `action_world.json` uses bundled KayKit walkers: knight (player), two rogues and a mage (minions), barbarian (boss). Props are crate pillars, tree, pine, rock, and well. `action::seed` parks the boss as `name == "wait"` until every minion is down, then wakes it. Win is minions plus boss. Confirm on the result screen reloads the seed (retry). Walker `wait` is not drawn; `ko` stays in the dump and is tinted.

The first scripted driver stopped inside attack range without turning (wish under the walk deadzone), so swings missed and kills stayed 0. Facing the nearest live body before the swing fixed it. Combo lands because held attacks retrigger inside `COMBO_TIME` (0.48s).

Shared edits stayed additive: `py/mod.rs`, `lib.rs` re-export, `kagra/__init__.py`, `FRONT_NAMES`, `tests/test_play_world.py`, and a 5-line skip in the walker draw loop.

Merged `origin/master` (rhythm and tower-defense sessions) before the PR. `genre_session.rs` was rebuilt from master's file plus the action and fight adapters, so `ActionSession`, `FightSession`, `RhythmSession`, and `TdSession` all stay registered.

A single-process verify loop reused the cached offscreen renderer and reported `update_mesh: N out of range` on later scenes. One process per scenario (what `python -m kagra.verify` does) is clean.
