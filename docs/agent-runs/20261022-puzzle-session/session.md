# Session

`puzzle.rs` is only the crate-push room (`genre: puzzle_pad`). It has no grid, match, or sliding board. Those rules went in a new module `puzzle_board.rs` (`genre: puzzle`) so `WorldPlay` and `puzzle_pad` stay untouched. `PuzzleSession` implements `GenreSession` as a thin forwarder in `genre_session.rs`. Python gets `PuzzleSession` next to `PuzzlePadSession`.

Levels (authored, demo-solved in tests):

1. 3x3 slide, one move
2. 3x3 lights-out grid, two taps
3. 3x3 match, one swap that clears 6 (a swap with no line reverts)
4. 4x4 slide, four moves, then win

Retry reloads the current level (the finale, after a win). Confirm advances a cleared level. Arrows / WASD are a latched stick. Tap and swipe hit the board through the room camera. HUD text is ASCII hyphen only.

The first match "dud" swap actually made a column of three. The test now swaps the right edge, which does not match.

Lavapipe (`lvp_icd.json`) rendered the title board and the win frame. `puzzle_smoke` checks that center region (not a flat clear) and the dark indoor sky.
