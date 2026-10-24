# Session

Two workers shared one tree: the first owned the Rust side (specs, missions, fixture rebake, fmt/clippy, git and CI); the second owned the Python mirror, the page, the generated assets, the ledger and the docs. They talked through a scratch note. Nothing was committed by the second worker.

Where it got stuck:

- Both workers began on `atmosphere.rs` at once and the API collided. They split the work by file and agreed on one API (`View`, `wall_props`, `push_effects`) before going on.
- Sign strokes were invisible in the first render. Strokes a few centimetres in front of a far surface lose to the depth buffer. Fix: put the strokes 28 cm in front of the plate (plane 2.42, plate 2.7). Far signs still show only their plates because the line fade hides the strokes.
- The concrete wall was too bright and the plates were solid enough to hide their glyphs. Wall emissive went from 0.45 to 0.34 and the plate emissive to `glow * 0.2`.
- `.gitignore` ignores `*.png` and `*.wav`, so the generated assets are added with `git add -f`.
- The ledger check failed on CREDITS freshness until `credits --write` was run.
- The page audio can't be heard in CI. A node harness (`tests/data/page_audio_harness.mjs`) runs the page's own audio block against a fake Web Audio and checks the unlock, the files, the volumes and the BGM fade.
