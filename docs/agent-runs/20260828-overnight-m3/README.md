# Overnight M3 try index — 2026-08-28

Compare: https://github.com/EMMA019/KAGRA/compare/master...local/m3-survival-meter

origin/master is still 9792702 (#106 collectathon). **Not merged.** gh is missing from PATH, so no PR numbers for these stacked slices.

Default Crest still: python -m kagra.play_world

Stacked from #106 master to HEAD, oldest first. Try from repo root.

| # | branch | closed | try |
|---|--------|--------|-----|
| 1 | local/m3-action-hit-die-retry | action hit/die/retry | python -m kagra.play_world kagra-shared/tests/fixtures/action_arena_world.json |
| 2 | local/m3-platformer-jump-checkpoint | platformer jump/land/checkpoint | python -m kagra.play_world kagra-shared/tests/fixtures/box_hop_world.json |
| 3 | local/m3-rpg-talk-scene | RPG talk, flag, town/dungeon | python -m kagra.play_world kagra-shared/tests/fixtures/rpg_town_world.json |
| 4 | local/m3-sprite-worlddoc | 2D sprite/quad on the same WorldDoc | python -m kagra.play_world kagra-shared/tests/fixtures/sprite_card_world.json |
| 5 | local/m3-fps-look-fire | FPS look/fire/hit | python -m kagra.play_world kagra-shared/tests/fixtures/fps_range_world.json |
| 6 | local/m3-fps-first-person | FPS first-person eye camera | python -m kagra.play_world kagra-shared/tests/fixtures/fps_range_world.json |
| 7 | local/m3-td-path-spawn | TD path/spawn/hit | python -m kagra.play_world kagra-shared/tests/fixtures/td_lane_world.json |
| 8 | local/m3-race-drive | racing kinematic drive/lap | python -m kagra.play_world kagra-shared/tests/fixtures/race_drive_world.json |
| 9 | local/m3-fight-hitstun | fighting attack/hitstun/KO | python -m kagra.play_world kagra-shared/tests/fixtures/fight_hitstun_world.json |
| 10 | local/m3-novel-pages | novel pages + choice + flag | python -m kagra.play_world kagra-shared/tests/fixtures/novel_pages_world.json |
| 11 | local/m3-stealth-hide | stealth hide + detect + clear/caught | python -m kagra.play_world kagra-shared/tests/fixtures/stealth_hide_world.json |
| 12 | local/m3-shared-picture | shared wgpu 30 picture (key+fill, contact blob, metal GGX) | python -m kagra.play_world |
| 13 | local/m3-puzzle-pad | puzzle pad | python -m kagra.play_world kagra-shared/tests/fixtures/puzzle_pad_world.json |
| 14 | local/m3-sports-goal | sports goal | python -m kagra.play_world kagra-shared/tests/fixtures/sports_goal_world.json |
| 15 | local/m3-sim-meter | sim zone meter | python -m kagra.play_world kagra-shared/tests/fixtures/sim_meter_world.json |
| 16 | local/m3-2d-action | 2D action walk/hit/kill | python -m kagra.play_world kagra-shared/tests/fixtures/action_side_world.json |

SHAs on origin (do not invent): 436d08f, a3ff7c4, 6aa11bc, d1804c1, 5191437, fcb7c73, 3454a35, 6a7c6e9, 7da8f7b, e4fe1df, 75ff481, 3e8886f, 979f824, 37282f5, 2b67fd9, cf293f9.
