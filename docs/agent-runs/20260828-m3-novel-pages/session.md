# Session

- Start: `local/m3-fight-hitstun` `7da8f7b` (do not merge origin/master).
- Branch: `local/m3-novel-pages`.
- Add `kagra-shared/src/novel.rs` + room dump. Tiny WorldPlay / window dispatch.
- Verify: `cargo test -p kagra-shared --locked --offline --lib`; wasm32 `--features wasm,render`; `pytest -m "not golden"` if feasible.
- Try: `python -m kagra.play_world kagra-shared/tests/fixtures/novel_pages_world.json`.
- Push branch (gh not on PATH). Compare URL from origin.
