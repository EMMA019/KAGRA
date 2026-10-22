# Novel GenreSession

## Decisions

- `NovelSession` wraps `WorldPlay` the same way `PuzzlePadSession` does. `genre_id` stays `novel_pages` (existing `WorldDoc.genre`). No second tick loop, no genre module added to `WorldPlay`.
- The short story (speaker ユキ, two lines, 残る / 歩く) lives in `novel.rs`. Python only feeds taps and draws `hud_json`.
- A tap is its own edge. Key-hold repeat is unchanged. Choice buttons use the same rects as the HUD, so a touch on the button both selects and confirms.
- Glyphs: Latin stays on Public Sans when that face has the character. Otherwise PixelMplus. If neither face has a non-zero glyph id, nothing is drawn (no `.notdef` box).
- Stage fixture `novel_world.json`: dusk room, `background` wall, KayKit `knight` walker as the character. The capsule reader steps off camera when a `character` walker is present so it does not cover the portrait. `novel_pages_world.json` is unchanged.

## Verify

- `cargo test -p kagra-shared --lib --locked` — 406 passed
- clippy `-D warnings` default, `render`, and `python` — clean
- `pytest tests -m "not golden"` — passed
- Headless lavapipe (`VK_ICD_FILENAMES` = Mesa llvmpipe): `scratch/novel_minimal.png` shows ユキ / どうする? / 残る / 歩く, the knight, and the back wall. No tofu boxes.
