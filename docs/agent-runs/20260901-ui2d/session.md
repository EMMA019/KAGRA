# Session — 2D UI パネル（2026-09-01）

## 設計

- `kagra/ui2d.py`（純 Python、拡張非依存で import 可能）:
  - `merge(*parts)` — 部品（quads + texts）を 1 つの hud dict に合成
  - `panel` — 枠（border） + 中身の 2 quad
  - `measure(text, size)` — `kagra_shared.measure_text` 実測、無ければ近似
  - `wrap_text` — 実測幅で折り返し（`\n` は明示改行）
  - `message` — 折り返し + 自動高さ + 任意タイトル
  - `choice_menu` — 選択行にカーソル（">"）+ カーソル色
  - `bar` — 0..1 に clamp したプログレスバー + ラベル
  - `list_lines` — パネルなしの単純リスト（在庫・ステータス）
- shared 側の追加は最小: `font.rs` に free fn `measure_text`（h_advance のみ
  なのでキャッシュ不要）、`py.rs` に `kagra_shared.measure_text(text, size)`。
  ラスタライズや描画の変更は無し。

## 躓き

1. `_text_quads()` が**リスト**を返すのに、`message()` / `bar()` がそれを
   dict と混ぜて `merge(*parts)` に渡した → `AttributeError: 'list' object
   has no attribute 'get'`。`{"texts": _text_quads(...)}` で包んで修正。
   （テストは label なしの呼び方だったので通り、デモで初めて露呈）
2. 選択肢のカーソル文字は "▶" を避けて ">" にした（PixelMplus の記号
   カバレッジに依存しない安全策）。

## 検証

- pytest tests/test_ui2d.py: 7 件（幅・折り返し・パネル・カーソル・
  clamp・merge・draw_world 統合）。kagra_shared 無しでも純ロジックは通る。
- デモ `examples/ui_panel_demo.py` → scratch/ui_panel.png（8721 bytes）。
- RGBA 画素比較: hud 有無で **22654 画素**変化 — パネル + メッセージ +
  選択肢 + バーが実際に描画されている。
- `cargo test --features render font::` 7 件パス、clippy `-D warnings`
  クリーン、`pytest tests -m "not golden"` 全パス。
