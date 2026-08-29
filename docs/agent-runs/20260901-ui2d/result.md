# Result — 2D UI パネルスライス

## 成果物

- `kagra/ui2d.py` — 純 Python UI 部品: `merge` / `panel` / `measure` /
  `wrap_text` / `message` / `choice_menu` / `bar` / `list_lines`
- `kagra-shared/src/font.rs` — free fn `measure_text(text, size)`（実測幅、
  キャッシュ不要）
- `kagra-shared/src/py.rs` — `kagra_shared.measure_text(text, size)`
- `tests/test_ui2d.py` — 7 件
- `examples/ui_panel_demo.py` — トルネコ風 1 枚（メッセージ + 選択肢 +
  HP/EXP バー + 在庫リスト）をヘッドレス PNG に

## verify

- pytest 全パス / clippy `-D warnings` クリーン / font テスト 7 件パス
- RGBA 画素比較: UI で 22654 画素変化（描画を実証）
- `python examples/ui_panel_demo.py scratch/ui_panel.png` → 8721 bytes

## 使い方（トルネコの入口がこれで書ける）

```python
from kagra.ui2d import choice_menu, merge, message
from kagra.gameloop import draw_world

hud = merge(
    message("トルネコは 50G を手に入れた！", 40, 120, 240),
    choice_menu(["はい", "いいえ"], selected=0, x=40, y=90, w=240),
)
png = draw_world(world, 320, 180, hud=hud)
```

## 次の山

③ 音（tone / sound 合成 + Python 側再生）→ ④ 入力拡張（マウス）→
⑤ 1 本目ジャンル（バニーガーデン系）。
