# Result

`character_controller_slides_on_a_wall_and_stands_on_the_ground` and `character_controller_steps_onto_a_low_block` passed (`--features physics`). `sfx::` presets passed. Clippy `-D warnings` is clean with `physics`, without it, and with `--all-targets` (the CI job). `cargo check --target wasm32-unknown-unknown --features wasm,physics` and `--features wasm,render` both finished. `pytest tests/test_rigid.py::test_character_controller_slides_on_a_wall` skipped here because the installed extension has no `move_character` yet. `python tools/gen_api_index.py --check` stayed clean.
