# Result

`pytest tests/test_ui2d.py` passed, including the 9-slice and styled-button cases. Font tests passed: coverage quads are 0.5px, kana and kanji still rasterize, missing glyphs stay empty. `python3 tools/gen_api_index.py --check` is clean (`nine_slice` and `styled_button` are public). Clippy is clean with and without `render`. Wasm32 release `--features wasm,render --locked` finished.
