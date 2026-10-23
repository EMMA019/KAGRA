# Result

`juice::tests` (curves, hit-stop, squash, tween), `crash_holds_play_until_the_hitstop_ends`, `straight_flight_scores_and_a_wall_ends_the_run`, and `opening_is_slower_and_a_grace_miss_does_not_crash` passed. Clippy is clean with and without `render`. `pytest tests/test_juice.py` passed. Master already dropped the quad trail; this merge keeps that removal and adds the crash hit-stop.
