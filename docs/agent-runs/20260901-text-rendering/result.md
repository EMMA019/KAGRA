# Result — 文字描画スライス

## 成果物

- `kagra-shared/assets/PixelMplus10-Regular.ttf` + `PixelMplus-LICENSE.txt`
  （M+ ライセンス。itouhiro/PixelMplus より）
- `kagra-shared/src/font.rs` — `TextRaster`: グリフカバレッジ → 1px Quad 展開。
  `measure_text_width` / `text_quads`。埋め込みフォントは include_bytes。
- `kagra-shared/src/scene.rs` — `TextQuad` / `TextAlign`、`DrawList.texts`
- `kagra-shared/src/render/mod.rs` — `build_vertices` が texts を展開。
  `draw_world_doc_with_hud` / `render_world_doc_with_hud`（free fn 含む）
- `kagra-shared/src/py.rs` — `render_world_doc(json, w, h, hud_json=None)`。
  hud JSON = `{"quads":[{x,y,w,h,color}], "texts":[{text,x,y,size,color,align}]}`
- `kagra/gameloop.py` — `draw_world(world, w, h, hud=None)`
- `tests/test_gameloop.py` — `test_draw_world_hud_text_when_shared_installed`
- `examples/python_game_minimal.py` — HUD テキスト（「WASD: 歩く J: 釣る」）表示
- 既存テスト 2 件の修復（bloom.wgsl の ACES / MToon の location 12）

## verify

- Rust: lib 383 + offscreen 12 パス。clippy `-D warnings` クリーン。wasm32 OK。
- Python: `pytest tests -m "not golden"` パス。
- `python examples/python_game_minimal.py --headless scratch/py_game.png`
  → 日本語 HUD テキスト入り PNG 出力。

## 次の山

2D UI パネル（メッセージウィンドウ / 選択肢 / リスト / バー）を Python 側で
組み、`draw_world` の hud に変換する（`kagra/ui2d.py` 想定）。
