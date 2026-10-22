# Rhythm GenreSession

`rhythm.rs` is the one-marker beat slice (`genre: rhythm_beat`) on `WorldPlay`. It has a hit window and hit/miss, and no lanes, perfect/good split, combo, or score. Those rules went in `rhythm_chart.rs` (`genre: rhythm`) so `WorldPlay` and `rhythm_beat` stay untouched. `RhythmSession` implements `GenreSession` as a thin forwarder in `genre_session.rs`. Python gets `RhythmSession`.

Chart: 28 notes, 4 lanes, 120 BPM, about 10 seconds. Windows are 50 ms perfect and 100 ms good. A miss past the late window breaks combo. Score is 300 / 100 / 0. Auto-play hits inside the perfect window so headless and tests clear with rank S.

Keys D F J K are edge-triggered lane presses. `k` was missing from the gameloop keymap, so it is mapped. A tap splits the screen into four columns. Title and result confirm on tap. HUD text is ASCII hyphen only.

`assets/cute_song_trial.wav` plays when a desktop run starts. The chart clock does not read the file; timing stays the fixed BPM if playback is silent.

Lavapipe rendered the title highway, a mid-chart frame (combo 9, PERFECT), and the result (S, score 8400).

Merged `origin/master` after race and tower defense landed. Shared files keep all three sessions.
