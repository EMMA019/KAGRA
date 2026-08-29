# Session — 入力拡張（マウス、2026-09-01）

## 設計

- `kagra/gameloop.py` にマウス状態を追加:
  - `_mouse = {"x", "y", "buttons": set, "just": set}`
  - `_on_mouse_motion` / `_on_mouse_down`（num: 1/2/3）/ `_on_mouse_up`
  - `mouse_pos()` / `mouse_down(button)` / `mouse_clicked(button)`
  - run() の `_tick` で `_mouse["just"].clear()`（キーの `_just` と同じ）。
    `<Motion>` / `<Button-1..3>` / `<ButtonRelease-1..3>` をバインド。
- 座標は tkinter の event.x / event.y（窓 = Label なのでそのまま画面座標）。

## 判断

- ゲームパッド（gilrs）は 0.19 側の kagra_core 依存で、Python ゲームマスター
  に載せるには pygame 等の依存追加が必要。バニーガーデン / トルネコは
  キー + マウスで足りるので後回し（ロードマップの 80% 外ではないが優先度
  を下げた）。IME も同様。

## 検証

- pytest tests/test_gameloop.py: 5 件（+ マウスハンドラの状態遷移: 位置 /
  押下 / クリック瞬間 / 解放 / ボタン 3）。
