# Session — 2026-08-23 Prop texture + 1-level parent

ロードマップ部屋トラックの次項。ゲーム新規ではなく play surface の API。

## 判断

- `Prop(..., texture=id)`。0 なら今まで通り `color` → `solid_tex`。
- 親子は 1 段だけ。孫、`parent` が既に子、親が子になる、は `ValueError`。
- コンストラクタの `parent=` はローカル座標（`keep_world=False`）。
  後から付ける `set_parent` の既定は世界位置を保つ。
- XZ は `world_verts` と同じ回転。`hovered_prop` / 衝突ボディは世界座標。
- Prop Garden のスモーク画素を変えないため、チェッカー箱と金球の子は
  `KAGRA_SMOKE` では作らない。

## Verify

`kagra_core` がこの環境に無いので GPU シナリオは未実行。
`pytest tests -m "not golden"` と `python tools/gen_api_index.py --check`。
